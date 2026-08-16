#include "video_decoder.h"

#include "hilog/log.h"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <queue>
#include <string>
#include <thread>
#include <vector>

#include <multimedia/player_framework/native_avbuffer.h>
#include <multimedia/player_framework/native_avbuffer_info.h>
#include <multimedia/player_framework/native_avcodec_base.h>
#include <multimedia/player_framework/native_avcodec_videodecoder.h>
#include <multimedia/player_framework/native_averrors.h>
#include <multimedia/player_framework/native_avformat.h>
#include <native_window/external_window.h>
#include <arm_neon.h>

#undef LOG_DOMAIN
#define LOG_DOMAIN 0x3201
#undef LOG_TAG
#define LOG_TAG "HdDecoder"

namespace {
struct CodecBuffer {
    uint32_t index = 0;
    OH_AVBuffer *buffer = nullptr;
};

class BufferQueue {
public:
    void Push(CodecBuffer item)
    {
        std::lock_guard<std::mutex> lock(mu_);
        items_.push(item);
        cv_.notify_one();
    }

    bool Pop(CodecBuffer &item, int timeoutMs)
    {
        std::unique_lock<std::mutex> lock(mu_);
        if (!cv_.wait_for(lock, std::chrono::milliseconds(timeoutMs), [this]() { return !items_.empty() || flush_; })) {
            return false;
        }
        if (items_.empty()) {
            return false;
        }
        item = items_.front();
        items_.pop();
        return true;
    }

    void Flush()
    {
        std::lock_guard<std::mutex> lock(mu_);
        while (!items_.empty()) {
            items_.pop();
        }
        flush_ = true;
        cv_.notify_all();
    }

    void Reset()
    {
        std::lock_guard<std::mutex> lock(mu_);
        while (!items_.empty()) {
            items_.pop();
        }
        flush_ = false;
    }

private:
    std::mutex mu_;
    std::condition_variable cv_;
    std::queue<CodecBuffer> items_;
    bool flush_ = false;
};

HdPacketApi g_api{};
std::mutex g_frameMu;
std::mutex g_latestMu;
std::condition_variable g_convertCv;
std::thread g_convertThread;
std::vector<uint8_t> g_rgba;
int32_t g_outW = 0;
int32_t g_outH = 0;
std::atomic<bool> g_hasFrame{false};
std::atomic<bool> g_running{false};
std::thread g_thread;

OH_AVCodec *g_codec = nullptr;
std::string g_codecName;
int32_t g_cfgW = 0;
int32_t g_cfgH = 0;
int32_t g_stride = 0;
int32_t g_sliceH = 0;
int32_t g_pixFmt = AV_PIXEL_FORMAT_NV12;
BufferQueue g_inQ;
BufferQueue g_outQ;
int32_t g_lastSeq = 0;
int32_t g_lastKeySeq = 0;
std::string g_failedCodec;
bool g_syncMode = true;
bool g_gotKeyframe = false;
int32_t g_pushed = 0;
std::atomic<int32_t> g_decoded{0};
std::atomic<int32_t> g_copySeq{0};
std::atomic<uint64_t> g_wantedSurface{0};
uint64_t g_attachedSurface = 0;
OHNativeWindow *g_window = nullptr;
std::vector<uint8_t> g_pending;
bool g_pendingKey = false;
std::atomic<bool> g_needReset{false};
int32_t g_pushFail = 0;
constexpr int32_t kMaxPacket = 8 * 1024 * 1024;
std::vector<uint8_t> g_latestRaw;
int32_t g_latestW = 0;
int32_t g_latestH = 0;
int32_t g_latestStride = 0;
int32_t g_latestSlice = 0;
int32_t g_latestPix = AV_PIXEL_FORMAT_NV12;
bool g_haveLatest = false;
bool g_surfaceRender = false;
std::atomic<int32_t> g_lastConvertMs{0};
std::atomic<int64_t> g_lastProbeMs{0};

void Log(const char *line)
{
    OH_LOG_INFO(LOG_APP, "%{public}s", line);
}

void YuvToRgba8(uint8x8_t y8, uint8x8_t u8, uint8x8_t v8, uint8_t *out)
{
    const int16x8_t y16 = vreinterpretq_s16_u16(vmovl_u8(y8));
    const int16x8_t u16 = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(u8)), vdupq_n_s16(128));
    const int16x8_t v16 = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(v8)), vdupq_n_s16(128));
    const int32x4_t yLo = vmovl_s16(vget_low_s16(y16));
    const int32x4_t yHi = vmovl_s16(vget_high_s16(y16));
    const int32x4_t uLo = vmovl_s16(vget_low_s16(u16));
    const int32x4_t uHi = vmovl_s16(vget_high_s16(u16));
    const int32x4_t vLo = vmovl_s16(vget_low_s16(v16));
    const int32x4_t vHi = vmovl_s16(vget_high_s16(v16));
    const int32x4_t rLo = vaddq_s32(yLo, vshrq_n_s32(vmulq_n_s32(vLo, 359), 8));
    const int32x4_t rHi = vaddq_s32(yHi, vshrq_n_s32(vmulq_n_s32(vHi, 359), 8));
    const int32x4_t gLo =
        vsubq_s32(yLo, vshrq_n_s32(vaddq_s32(vmulq_n_s32(uLo, 88), vmulq_n_s32(vLo, 183)), 8));
    const int32x4_t gHi =
        vsubq_s32(yHi, vshrq_n_s32(vaddq_s32(vmulq_n_s32(uHi, 88), vmulq_n_s32(vHi, 183)), 8));
    const int32x4_t bLo = vaddq_s32(yLo, vshrq_n_s32(vmulq_n_s32(uLo, 454), 8));
    const int32x4_t bHi = vaddq_s32(yHi, vshrq_n_s32(vmulq_n_s32(uHi, 454), 8));
    uint8x8x4_t rgba;
    rgba.val[0] = vqmovun_s16(vcombine_s16(vqmovn_s32(rLo), vqmovn_s32(rHi)));
    rgba.val[1] = vqmovun_s16(vcombine_s16(vqmovn_s32(gLo), vqmovn_s32(gHi)));
    rgba.val[2] = vqmovun_s16(vcombine_s16(vqmovn_s32(bLo), vqmovn_s32(bHi)));
    rgba.val[3] = vdup_n_u8(255);
    vst4_u8(out, rgba);
}

void YuvSpToRgba(const uint8_t *src, int32_t width, int32_t height, int32_t stride, int32_t sliceH, uint8_t *dst,
    bool nv21)
{
    const int32_t yStride = stride > 0 ? stride : width;
    const int32_t yRows = sliceH > 0 ? sliceH : height;
    const uint8_t *yPlane = src;
    const uint8_t *uvPlane = src + yStride * yRows;
    for (int32_t y = 0; y < height; ++y) {
        const uint8_t *yRow = yPlane + y * yStride;
        const uint8_t *uvRow = uvPlane + (y / 2) * yStride;
        uint8_t *out = dst + static_cast<size_t>(y * width * 4);
        int32_t x = 0;
        for (; x + 8 <= width; x += 8) {
            const uint8x8_t y8 = vld1_u8(yRow + x);
            const uint8x8_t uv8 = vld1_u8(uvRow + x);
            const uint8x8x2_t uz = vuzp_u8(uv8, uv8);
            const uint8x8_t first = vzip_u8(uz.val[0], uz.val[0]).val[0];
            const uint8x8_t second = vzip_u8(uz.val[1], uz.val[1]).val[0];
            const uint8x8_t u8 = nv21 ? second : first;
            const uint8x8_t v8 = nv21 ? first : second;
            YuvToRgba8(y8, u8, v8, out + x * 4);
        }
        for (; x < width; ++x) {
            const int yv = yRow[x];
            const int uvIndex = x & ~1;
            const int u = (nv21 ? uvRow[uvIndex + 1] : uvRow[uvIndex]) - 128;
            const int v = (nv21 ? uvRow[uvIndex] : uvRow[uvIndex + 1]) - 128;
            int r = yv + ((v * 359) >> 8);
            int g = yv - ((u * 88 + v * 183) >> 8);
            int b = yv + ((u * 454) >> 8);
            uint8_t *px = out + x * 4;
            px[0] = static_cast<uint8_t>(std::clamp(r, 0, 255));
            px[1] = static_cast<uint8_t>(std::clamp(g, 0, 255));
            px[2] = static_cast<uint8_t>(std::clamp(b, 0, 255));
            px[3] = 255;
        }
    }
}

void Nv12ToRgba(const uint8_t *src, int32_t width, int32_t height, int32_t stride, int32_t sliceH, uint8_t *dst)
{
    YuvSpToRgba(src, width, height, stride, sliceH, dst, false);
}

void Nv21ToRgba(const uint8_t *src, int32_t width, int32_t height, int32_t stride, int32_t sliceH, uint8_t *dst)
{
    YuvSpToRgba(src, width, height, stride, sliceH, dst, true);
}

void OnError(OH_AVCodec *, int32_t errorCode, void *)
{
    char line[96];
    std::snprintf(line, sizeof(line), "decoder error %d", static_cast<int>(errorCode));
    Log(line);
    g_needReset.store(true);
}

void OnStreamChanged(OH_AVCodec *, OH_AVFormat *format, void *)
{
    if (format == nullptr) {
        return;
    }
    int32_t width = 0;
    int32_t height = 0;
    int32_t stride = 0;
    int32_t sliceH = 0;
    int32_t pix = 0;
    OH_AVFormat_GetIntValue(format, OH_MD_KEY_WIDTH, &width);
    OH_AVFormat_GetIntValue(format, OH_MD_KEY_HEIGHT, &height);
    OH_AVFormat_GetIntValue(format, OH_MD_KEY_VIDEO_STRIDE, &stride);
    OH_AVFormat_GetIntValue(format, OH_MD_KEY_VIDEO_SLICE_HEIGHT, &sliceH);
    OH_AVFormat_GetIntValue(format, OH_MD_KEY_PIXEL_FORMAT, &pix);
    if (width > 0) {
        g_cfgW = width;
    }
    if (height > 0) {
        g_cfgH = height;
    }
    if (stride > 0) {
        g_stride = stride;
    }
    if (sliceH > 0) {
        g_sliceH = sliceH;
    }
    if (pix > 0) {
        g_pixFmt = pix;
    }
    char line[128];
    std::snprintf(line, sizeof(line), "decoder stream %dx%d stride=%d slice=%d pix=%d", g_cfgW, g_cfgH, g_stride,
        g_sliceH, g_pixFmt);
    Log(line);
}

void OnNeedInput(OH_AVCodec *, uint32_t index, OH_AVBuffer *buffer, void *)
{
    g_inQ.Push({index, buffer});
}

void OnNewOutput(OH_AVCodec *, uint32_t index, OH_AVBuffer *buffer, void *)
{
    g_outQ.Push({index, buffer});
}

const char *MimeForCodec(const std::string &name)
{
    if (name == "h265") {
        return OH_AVCODEC_MIMETYPE_VIDEO_HEVC;
    }
    if (name == "h264") {
        return OH_AVCODEC_MIMETYPE_VIDEO_AVC;
    }
    return nullptr;
}

bool IsAnnexB(const uint8_t *data, size_t len)
{
    return len >= 3 && data[0] == 0 && data[1] == 0 && (data[2] == 1 || (len >= 4 && data[2] == 0 && data[3] == 1));
}

std::vector<uint8_t> ToAnnexB(const uint8_t *data, size_t len)
{
    if (data == nullptr || len == 0) {
        return {};
    }
    if (IsAnnexB(data, len)) {
        return {data, data + len};
    }
    std::vector<uint8_t> out;
    out.reserve(len + 16);
    size_t i = 0;
    size_t nals = 0;
    while (i + 4 <= len) {
        const uint32_t n = (static_cast<uint32_t>(data[i]) << 24) | (static_cast<uint32_t>(data[i + 1]) << 16) |
            (static_cast<uint32_t>(data[i + 2]) << 8) | static_cast<uint32_t>(data[i + 3]);
        if (n == 0 || i + 4 + n > len) {
            break;
        }
        i += 4;
        out.insert(out.end(), {0, 0, 0, 1});
        out.insert(out.end(), data + i, data + i + n);
        i += n;
        nals++;
    }
    if (nals > 0 && i == len) {
        return out;
    }
    out.clear();
    out.insert(out.end(), {0, 0, 0, 1});
    out.insert(out.end(), data, data + len);
    return out;
}

void UpdateOutputFormat();
void DestroyCodec();
void DestroyWindow();
bool CreateCodec(const std::string &name, int32_t width, int32_t height);

void UpdateOutputFormat()
{
    if (g_codec == nullptr) {
        return;
    }
    OH_AVFormat *format = OH_VideoDecoder_GetOutputDescription(g_codec);
    if (format == nullptr) {
        return;
    }
    OnStreamChanged(g_codec, format, nullptr);
    OH_AVFormat_Destroy(format);
}

void DestroyWindow()
{
    if (g_window == nullptr) {
        return;
    }
    OH_NativeWindow_DestroyNativeWindow(g_window);
    g_window = nullptr;
    g_attachedSurface = 0;
}

void ApplyWindow(int32_t width, int32_t height)
{
    if (g_window == nullptr) {
        return;
    }
    const int32_t w = width > 0 ? width : 1920;
    const int32_t h = height > 0 ? height : 1080;
    OH_NativeWindow_NativeWindowHandleOpt(g_window, SET_BUFFER_GEOMETRY, w, h);
    OH_NativeWindow_NativeWindowSetScalingModeV2(g_window, OH_SCALING_MODE_SCALE_FIT_V2);
}

void AttachWantedSurface()
{
    const uint64_t wanted = g_wantedSurface.load();
    if (wanted == g_attachedSurface && (wanted == 0 || g_window != nullptr)) {
        return;
    }
    const std::string codecName = g_codecName;
    const int32_t dw = g_api.displayWidth != nullptr ? g_api.displayWidth() : 0;
    const int32_t dh = g_api.displayHeight != nullptr ? g_api.displayHeight() : 0;
    if (g_codec != nullptr) {
        DestroyCodec();
    }
    DestroyWindow();
    g_surfaceRender = false;
    if (wanted == 0) {
        Log("video surface cleared");
        return;
    }
    OHNativeWindow *win = nullptr;
    const int32_t err = OH_NativeWindow_CreateNativeWindowFromSurfaceId(wanted, &win);
    if (err != 0 || win == nullptr) {
        char line[96];
        std::snprintf(line, sizeof(line), "native window failed %d", static_cast<int>(err));
        Log(line);
        return;
    }
    g_window = win;
    g_attachedSurface = wanted;
    ApplyWindow(dw, dh);
    char line[128];
    std::snprintf(line, sizeof(line), "video surface attached id=%llu", static_cast<unsigned long long>(wanted));
    Log(line);
    if (!codecName.empty()) {
        CreateCodec(codecName, dw, dh);
    }
    if (g_api.requestKey != nullptr) {
        g_api.requestKey();
    }
}

void DestroyCodec()
{
    if (g_codec == nullptr) {
        return;
    }
    OH_VideoDecoder_Stop(g_codec);
    OH_VideoDecoder_Destroy(g_codec);
    g_codec = nullptr;
    g_inQ.Flush();
    g_outQ.Flush();
    g_inQ.Reset();
    g_outQ.Reset();
    g_gotKeyframe = false;
    g_pushed = 0;
    g_decoded.store(0);
    {
        std::lock_guard<std::mutex> lock(g_latestMu);
        g_haveLatest = false;
        g_latestRaw.clear();
    }
}

bool ConfigureAndStart(int32_t width, int32_t height, int32_t pix)
{
    OH_AVFormat *format = OH_AVFormat_Create();
    if (format == nullptr) {
        return false;
    }
    const int32_t w = width > 0 ? width : 1920;
    const int32_t h = height > 0 ? height : 1080;
    OH_AVFormat_SetIntValue(format, OH_MD_KEY_WIDTH, w);
    OH_AVFormat_SetIntValue(format, OH_MD_KEY_HEIGHT, h);
    OH_AVFormat_SetIntValue(format, OH_MD_KEY_FRAME_RATE, 30);
    OH_AVFormat_SetIntValue(format, OH_MD_KEY_MAX_INPUT_SIZE, kMaxPacket);
    OH_AVFormat_SetIntValue(format, OH_MD_KEY_VIDEO_ENABLE_LOW_LATENCY, 1);
    if (pix > 0) {
        OH_AVFormat_SetIntValue(format, OH_MD_KEY_PIXEL_FORMAT, pix);
    }
    int32_t cfg = OH_VideoDecoder_Configure(g_codec, format);
    if (cfg != AV_ERR_OK) {
        OH_AVFormat_SetIntValue(format, OH_MD_KEY_VIDEO_ENABLE_LOW_LATENCY, 0);
        cfg = OH_VideoDecoder_Configure(g_codec, format);
    }
    OH_AVFormat_Destroy(format);
    if (cfg != AV_ERR_OK) {
        char line[96];
        std::snprintf(line, sizeof(line), "decoder configure failed %d pix=%d", static_cast<int>(cfg), pix);
        Log(line);
        return false;
    }
    g_surfaceRender = false;
    if (g_window != nullptr) {
        ApplyWindow(w, h);
        const int32_t surf = OH_VideoDecoder_SetSurface(g_codec, g_window);
        if (surf == AV_ERR_OK) {
            g_surfaceRender = true;
            Log("decoder surface mode (rustdesk-like)");
        } else {
            char line[96];
            std::snprintf(line, sizeof(line), "decoder set surface failed %d, buffer fallback", static_cast<int>(surf));
            Log(line);
        }
    }
    if (OH_VideoDecoder_Prepare(g_codec) != AV_ERR_OK || OH_VideoDecoder_Start(g_codec) != AV_ERR_OK) {
        Log("decoder start failed");
        return false;
    }
    g_cfgW = w;
    g_cfgH = h;
    g_stride = 0;
    g_sliceH = 0;
    g_pixFmt = pix > 0 ? pix : AV_PIXEL_FORMAT_NV12;
    char line[96];
    std::snprintf(line, sizeof(line), "decoder configure pix=%d surface=%d", g_pixFmt, g_surfaceRender ? 1 : 0);
    Log(line);
    return true;
}

bool OpenCodec(const char *mime, int32_t width, int32_t height, int32_t pix, bool async)
{
    DestroyCodec();
    g_codec = OH_VideoDecoder_CreateByMime(mime);
    if (g_codec == nullptr) {
        return false;
    }
    if (async) {
        OH_AVCodecCallback cb{};
        cb.onError = OnError;
        cb.onStreamChanged = OnStreamChanged;
        cb.onNeedInputBuffer = OnNeedInput;
        cb.onNewOutputBuffer = OnNewOutput;
        if (OH_VideoDecoder_RegisterCallback(g_codec, cb, nullptr) != AV_ERR_OK) {
            DestroyCodec();
            return false;
        }
    }
    g_syncMode = !async;
    if (!ConfigureAndStart(width, height, pix)) {
        DestroyCodec();
        return false;
    }
    return true;
}

bool CreateCodec(const std::string &name, int32_t width, int32_t height)
{
    const char *mime = MimeForCodec(name);
    if (mime == nullptr) {
        char line[96];
        std::snprintf(line, sizeof(line), "decoder skip unsupported codec %s", name.c_str());
        Log(line);
        return false;
    }
    // Surface path (RustDesk Texture analog): omit pixel format first, then NV12.
    // Buffer path: RGBA then NV12.
    std::vector<int32_t> pixTry;
    if (g_window != nullptr) {
        pixTry.push_back(0);
        pixTry.push_back(AV_PIXEL_FORMAT_NV12);
    } else {
        pixTry.push_back(AV_PIXEL_FORMAT_RGBA);
        pixTry.push_back(AV_PIXEL_FORMAT_NV12);
    }
    for (int32_t pix : pixTry) {
        if (!OpenCodec(mime, width, height, pix, false)) {
            continue;
        }
        uint32_t probeIndex = 0;
        const int32_t probe = OH_VideoDecoder_QueryInputBuffer(g_codec, &probeIndex, 50000);
        if (probe == AV_ERR_OPERATE_NOT_PERMIT) {
            Log("decoder sync not permitted, fallback async");
            if (!OpenCodec(mime, width, height, pix, true)) {
                continue;
            }
        } else if (probe == AV_ERR_OK) {
            OH_AVBuffer *buffer = OH_VideoDecoder_GetInputBuffer(g_codec, probeIndex);
            if (buffer != nullptr) {
                g_inQ.Push({probeIndex, buffer});
            }
        }
        g_codecName = name;
        g_gotKeyframe = false;
        char line[96];
        std::snprintf(line, sizeof(line), "decoder started %s %dx%d %s pix=%d surface=%d", name.c_str(), g_cfgW,
            g_cfgH, g_syncMode ? "sync" : "async", g_pixFmt, g_surfaceRender ? 1 : 0);
        Log(line);
        return true;
    }
    char line[96];
    std::snprintf(line, sizeof(line), "decoder create failed %s", name.c_str());
    Log(line);
    return false;
}

void StoreRgba(const uint8_t *src, int32_t width, int32_t height, int32_t stride, int32_t sliceH, int32_t pixFmt)
{
    if (src == nullptr || width <= 0 || height <= 0) {
        return;
    }
    std::vector<uint8_t> rgba(static_cast<size_t>(width * height * 4));
    if (pixFmt == AV_PIXEL_FORMAT_RGBA) {
        int32_t rowStride = stride > 0 ? stride : width * 4;
        if (rowStride < width * 4) {
            rowStride = width * 4;
        }
        for (int32_t y = 0; y < height; ++y) {
            std::memcpy(rgba.data() + static_cast<size_t>(y * width * 4), src + static_cast<size_t>(y * rowStride),
                static_cast<size_t>(width * 4));
        }
    } else if (pixFmt == AV_PIXEL_FORMAT_NV21) {
        Nv21ToRgba(src, width, height, stride, sliceH, rgba.data());
    } else {
        Nv12ToRgba(src, width, height, stride, sliceH, rgba.data());
    }
    std::lock_guard<std::mutex> lock(g_frameMu);
    g_rgba.swap(rgba);
    g_outW = width;
    g_outH = height;
    g_hasFrame.store(true);
    g_decoded.fetch_add(1);
}

void KeepLatestOutput(uint32_t index, OH_AVBuffer *buffer)
{
    if (g_codec == nullptr) {
        return;
    }
    const int32_t width = g_cfgW > 0 ? g_cfgW : 320;
    const int32_t height = g_cfgH > 0 ? g_cfgH : 180;
    // RustDesk Texture path: render decoded frame to NativeWindow / XComponent.
    if (g_surfaceRender && g_window != nullptr) {
        const int32_t ret = OH_VideoDecoder_RenderOutputBuffer(g_codec, index);
        if (ret != AV_ERR_OK) {
            char line[96];
            std::snprintf(line, sizeof(line), "decoder render failed %d", static_cast<int>(ret));
            Log(line);
            OH_VideoDecoder_FreeOutputBuffer(g_codec, index);
            return;
        }
        {
            std::lock_guard<std::mutex> lock(g_frameMu);
            g_outW = width;
            g_outH = height;
        }
        g_hasFrame.store(true);
        g_decoded.fetch_add(1);
        if (g_decoded.load() <= 3 || g_decoded.load() % 30 == 0) {
            char line[96];
            std::snprintf(line, sizeof(line), "decoder render out %d %dx%d", g_decoded.load(), width, height);
            Log(line);
        }
        return;
    }
    if (buffer == nullptr) {
        OH_VideoDecoder_FreeOutputBuffer(g_codec, index);
        return;
    }
    OH_AVCodecBufferAttr attr{};
    OH_AVBuffer_GetBufferAttr(buffer, &attr);
    uint8_t *addr = OH_AVBuffer_GetAddr(buffer);
    if (addr != nullptr && attr.size > 0) {
        if (g_stride == 0) {
            UpdateOutputFormat();
        }
        std::lock_guard<std::mutex> lock(g_latestMu);
        g_latestRaw.assign(addr, addr + attr.size);
        g_latestW = width;
        g_latestH = height;
        g_latestStride = g_stride;
        g_latestSlice = g_sliceH;
        g_latestPix = g_pixFmt;
        g_haveLatest = true;
    }
    OH_VideoDecoder_FreeOutputBuffer(g_codec, index);
    g_convertCv.notify_one();
}

void ConvertLoop()
{
    Log("convert thread start");
    while (g_running.load()) {
        if (g_surfaceRender) {
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
            continue;
        }
        std::vector<uint8_t> raw;
        int32_t width = 0;
        int32_t height = 0;
        int32_t stride = 0;
        int32_t sliceH = 0;
        int32_t pix = AV_PIXEL_FORMAT_NV12;
        {
            std::unique_lock<std::mutex> lock(g_latestMu);
            g_convertCv.wait_for(lock, std::chrono::milliseconds(8), [] {
                return !g_running.load() || g_haveLatest;
            });
            if (!g_running.load()) {
                break;
            }
            if (!g_haveLatest) {
                continue;
            }
            raw.swap(g_latestRaw);
            width = g_latestW;
            height = g_latestH;
            stride = g_latestStride;
            sliceH = g_latestSlice;
            pix = g_latestPix;
            g_haveLatest = false;
        }
        if (raw.empty()) {
            continue;
        }
        const auto started = std::chrono::steady_clock::now();
        StoreRgba(raw.data(), width, height, stride, sliceH, pix);
        const int32_t ms = static_cast<int32_t>(
            std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - started).count());
        g_lastConvertMs.store(ms);
        if (g_decoded.load() <= 3 || g_decoded.load() % 30 == 0 || ms >= 40) {
            char line[128];
            std::snprintf(line, sizeof(line), "decoder out %d %dx%d size=%d pix=%d convert=%dms", g_decoded.load(),
                width, height, static_cast<int>(raw.size()), pix, ms);
            Log(line);
        }
    }
    Log("convert thread end");
}

void DrainOutput()
{
    if (g_codec == nullptr) {
        return;
    }
    if (g_syncMode) {
        for (;;) {
            uint32_t index = 0;
            const int32_t err = OH_VideoDecoder_QueryOutputBuffer(g_codec, &index, 0);
            if (err == AV_ERR_STREAM_CHANGED) {
                UpdateOutputFormat();
                continue;
            }
            if (err != AV_ERR_OK) {
                break;
            }
            KeepLatestOutput(index, OH_VideoDecoder_GetOutputBuffer(g_codec, index));
        }
        return;
    }
    CodecBuffer item;
    while (g_outQ.Pop(item, 0)) {
        KeepLatestOutput(item.index, item.buffer);
    }
}

bool FillInputBuffer(uint32_t index, OH_AVBuffer *buffer, const std::vector<uint8_t> &packet, bool key)
{
    if (g_codec == nullptr || buffer == nullptr) {
        return false;
    }
    uint8_t *addr = OH_AVBuffer_GetAddr(buffer);
    const int32_t cap = OH_AVBuffer_GetCapacity(buffer);
    if (addr == nullptr || cap < static_cast<int32_t>(packet.size())) {
        char line[96];
        std::snprintf(line, sizeof(line), "decoder input too small cap=%d need=%d", static_cast<int>(cap),
            static_cast<int>(packet.size()));
        Log(line);
        OH_AVCodecBufferAttr skip{};
        skip.size = 0;
        OH_AVBuffer_SetBufferAttr(buffer, &skip);
        OH_VideoDecoder_PushInputBuffer(g_codec, index);
        return false;
    }
    std::memcpy(addr, packet.data(), packet.size());
    OH_AVCodecBufferAttr attr{};
    attr.size = static_cast<int32_t>(packet.size());
    attr.offset = 0;
    attr.pts = static_cast<int64_t>(g_pushed) * 33333;
    attr.flags = key ? AVCODEC_BUFFER_FLAGS_SYNC_FRAME : AVCODEC_BUFFER_FLAGS_NONE;
    OH_AVBuffer_SetBufferAttr(buffer, &attr);
    const int32_t ret = OH_VideoDecoder_PushInputBuffer(g_codec, index);
    if (ret != AV_ERR_OK) {
        char line[96];
        std::snprintf(line, sizeof(line), "decoder push failed %d", static_cast<int>(ret));
        Log(line);
        return false;
    }
    g_pushFail = 0;
    g_pushed++;
    if (key) {
        g_gotKeyframe = true;
    }
    if (g_pushed <= 5 || g_pushed % 30 == 0) {
        char line[128];
        std::snprintf(line, sizeof(line), "decoder in %d bytes=%d key=%d hex=%02x%02x%02x%02x%02x%02x", g_pushed,
            attr.size, key ? 1 : 0, packet[0], packet[1], packet[2], packet.size() > 3 ? packet[3] : 0,
            packet.size() > 4 ? packet[4] : 0, packet.size() > 5 ? packet[5] : 0);
        Log(line);
    }
    return true;
}

bool PushPending()
{
    if (g_codec == nullptr || g_pending.empty()) {
        return false;
    }
    if (g_syncMode) {
        CodecBuffer queued;
        if (g_inQ.Pop(queued, 0)) {
            return FillInputBuffer(queued.index, queued.buffer, g_pending, g_pendingKey);
        }
        uint32_t index = 0;
        const int32_t err = OH_VideoDecoder_QueryInputBuffer(g_codec, &index, 80000);
        if (err == AV_ERR_TRY_AGAIN_LATER) {
            return false;
        }
        if (err != AV_ERR_OK) {
            if (g_pushed < 3) {
                char line[96];
                std::snprintf(line, sizeof(line), "decoder query in %d", static_cast<int>(err));
                Log(line);
            }
            return false;
        }
        OH_AVBuffer *buffer = OH_VideoDecoder_GetInputBuffer(g_codec, index);
        return FillInputBuffer(index, buffer, g_pending, g_pendingKey);
    }
    CodecBuffer item;
    if (!g_inQ.Pop(item, 0)) {
        DrainOutput();
        if (!g_inQ.Pop(item, 0)) {
            return false;
        }
    }
    return FillInputBuffer(item.index, item.buffer, g_pending, g_pendingKey);
}

void DecoderLoop()
{
    Log("decoder thread start");
    std::vector<uint8_t> packet(static_cast<size_t>(kMaxPacket));
    while (g_running.load()) {
        AttachWantedSurface();
        if (g_needReset.exchange(false)) {
            Log("decoder reset after error");
            const std::string name = g_codecName;
            const int32_t dw = g_api.displayWidth != nullptr ? g_api.displayWidth() : 0;
            const int32_t dh = g_api.displayHeight != nullptr ? g_api.displayHeight() : 0;
            DestroyCodec();
            g_gotKeyframe = false;
            g_lastKeySeq = 0;
            g_lastSeq = 0;
            g_pending.clear();
            g_pushFail = 0;
            if (!name.empty()) {
                CreateCodec(name, dw, dh);
            }
            if (g_api.requestKey != nullptr) {
                g_api.requestKey();
            }
        }
        if (g_api.seq == nullptr) {
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
            continue;
        }
        const int32_t seq = g_api.seq != nullptr ? g_api.seq() : 0;
        (void)seq;
        int pulled = 0;
        while (pulled < 24) {
            if (g_pending.empty() && g_api.copyPacket != nullptr) {
                char codecName[16] = {0};
                const int32_t n = g_api.copyPacket(packet.data(), static_cast<int32_t>(packet.size()));
                if (n > 0) {
                    if (g_api.copyCodec != nullptr) {
                        g_api.copyCodec(codecName, sizeof(codecName));
                    }
                    const std::string name = codecName;
                    const bool key = g_api.key != nullptr && g_api.key() != 0;
                    if (name == g_failedCodec) {
                        DrainOutput();
                        break;
                    }
                    if (g_codec == nullptr || g_codecName != name) {
                        const int32_t dw = g_api.displayWidth != nullptr ? g_api.displayWidth() : 0;
                        const int32_t dh = g_api.displayHeight != nullptr ? g_api.displayHeight() : 0;
                        if (!CreateCodec(name, dw, dh)) {
                            g_failedCodec = name;
                            break;
                        }
                        g_failedCodec.clear();
                    }
                    g_pending = ToAnnexB(packet.data(), static_cast<size_t>(n));
                    g_pendingKey = key;
                }
            }
            if (g_pending.empty()) {
                break;
            }
            if (!PushPending()) {
                break;
            }
            if (g_pendingKey) {
                Log("decoder got keyframe");
            }
            g_pending.clear();
            pulled++;
            DrainOutput();
        }
        DrainOutput();
        if (g_pending.empty() && pulled == 0) {
            std::this_thread::sleep_for(std::chrono::milliseconds(g_gotKeyframe ? 1 : 8));
        }
        const int64_t nowMs = std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now().time_since_epoch())
                                  .count();
        if (nowMs - g_lastProbeMs.load() >= 1000) {
            g_lastProbeMs.store(nowMs);
            char line[160];
            std::snprintf(line, sizeof(line),
                "HdProbe in=%d out=%d surface=%d cvt=%dms cfg=%dx%d", g_pushed, g_decoded.load(),
                g_surfaceRender ? 1 : 0, g_lastConvertMs.load(), g_cfgW, g_cfgH);
            Log(line);
        }
        if (g_pending.empty() && g_pushed > 8 && g_decoded.load() <= 1 && g_pushed % 12 == 0) {
            char line[96];
            std::snprintf(line, sizeof(line), "decoder stalled in=%d out=%d, request key", g_pushed,
                g_decoded.load());
            Log(line);
            g_gotKeyframe = false;
            if (g_api.requestKey != nullptr) {
                g_api.requestKey();
            }
        }
    }
    DestroyCodec();
    DestroyWindow();
    Log("decoder thread end");
}
} // namespace

void HdDecoderBind(const HdPacketApi &api)
{
    g_api = api;
}

void HdDecoderStart()
{
    if (g_running.exchange(true)) {
        return;
    }
    g_lastSeq = 0;
    g_lastKeySeq = 0;
    g_failedCodec.clear();
    g_pending.clear();
    g_pendingKey = false;
    g_gotKeyframe = false;
    g_needReset.store(false);
    g_pushFail = 0;
    g_hasFrame.store(false);
    g_decoded.store(0);
    g_copySeq.store(0);
    {
        std::lock_guard<std::mutex> lock(g_frameMu);
        g_rgba.clear();
        g_outW = 0;
        g_outH = 0;
    }
    g_convertThread = std::thread(ConvertLoop);
    g_thread = std::thread(DecoderLoop);
}

void HdDecoderStop()
{
    if (!g_running.exchange(false)) {
        return;
    }
    g_inQ.Flush();
    g_outQ.Flush();
    g_convertCv.notify_all();
    if (g_thread.joinable()) {
        g_thread.join();
    }
    if (g_convertThread.joinable()) {
        g_convertThread.join();
    }
    g_inQ.Reset();
    g_outQ.Reset();
    g_hasFrame.store(false);
}

bool HdDecoderHasFrame()
{
    return g_hasFrame.load();
}

bool HdDecoderPeek(int32_t *width, int32_t *height)
{
    std::lock_guard<std::mutex> lock(g_frameMu);
    if (g_rgba.empty() || g_outW <= 0 || g_outH <= 0) {
        return false;
    }
    if (width != nullptr) {
        *width = g_outW;
    }
    if (height != nullptr) {
        *height = g_outH;
    }
    return true;
}

bool HdDecoderHasNewFrame()
{
    std::lock_guard<std::mutex> lock(g_frameMu);
    return !g_rgba.empty() && g_decoded.load() != g_copySeq.load();
}

bool HdDecoderCopyRgba(uint8_t *out, size_t outLen, int32_t *width, int32_t *height)
{
    std::lock_guard<std::mutex> lock(g_frameMu);
    if (g_rgba.empty() || g_outW <= 0 || g_outH <= 0) {
        return false;
    }
    const size_t need = static_cast<size_t>(g_outW * g_outH * 4);
    if (out == nullptr || outLen < need) {
        return false;
    }
    std::memcpy(out, g_rgba.data(), need);
    g_copySeq.store(g_decoded.load());
    g_convertCv.notify_one();
    if (width != nullptr) {
        *width = g_outW;
    }
    if (height != nullptr) {
        *height = g_outH;
    }
    return true;
}

void HdDecoderSetSurface(uint64_t surfaceId)
{
    g_wantedSurface.store(surfaceId);
}

void HdDecoderClearSurface()
{
    g_wantedSurface.store(0);
}

int32_t HdDecoderDecodedCount()
{
    return g_decoded.load();
}

int32_t HdDecoderPushedCount()
{
    return g_pushed;
}

int32_t HdDecoderSurfaceActive()
{
    return g_surfaceRender && g_window != nullptr ? 1 : 0;
}

int32_t HdDecoderLastConvertMs()
{
    return g_lastConvertMs.load();
}
