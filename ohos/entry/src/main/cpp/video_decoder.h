#ifndef HARMONYDESK_VIDEO_DECODER_H
#define HARMONYDESK_VIDEO_DECODER_H

#include <cstddef>
#include <cstdint>

struct HdPacketApi {
    int32_t (*seq)();
    int32_t (*key)();
    int32_t (*keySeq)();
    int32_t (*copyCodec)(char *, int32_t);
    int32_t (*copyPacket)(uint8_t *, int32_t);
    int32_t (*copyKeyPacket)(uint8_t *, int32_t);
    int32_t (*displayWidth)();
    int32_t (*displayHeight)();
    void (*requestKey)();
};

void HdDecoderBind(const HdPacketApi &api);
void HdDecoderStart();
void HdDecoderStop();
void HdDecoderSetSurface(uint64_t surfaceId);
void HdDecoderClearSurface();
int32_t HdDecoderDecodedCount();
int32_t HdDecoderPushedCount();
int32_t HdDecoderSurfaceActive(); // 1 = RustDesk-style surface render
int32_t HdDecoderLastConvertMs();
bool HdDecoderHasFrame();
bool HdDecoderPeek(int32_t *width, int32_t *height);
bool HdDecoderHasNewFrame();
bool HdDecoderCopyRgba(uint8_t *out, size_t outLen, int32_t *width, int32_t *height);

#endif
