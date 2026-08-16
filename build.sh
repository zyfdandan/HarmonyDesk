#!/bin/bash
# HarmonyDesk 构建脚本
# 用于编译 Rust Native 模块并构建鸿蒙应用

set -e  # 遇到错误立即退出

# 颜色输出
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  HarmonyDesk 构建脚本${NC}"
echo -e "${GREEN}========================================${NC}"

# 检查环境变量
echo -e "\n${YELLOW}检查环境变量...${NC}"

if [ -z "$HARMONYOS_NDK_PATH" ]; then
    echo -e "${RED}错误: 未设置 HARMONYOS_NDK_PATH 环境变量${NC}"
    echo "请设置环境变量指向鸿蒙 NDK 路径"
    echo "例如: export HARMONYOS_NDK_PATH=/path/to/ohos-ndk"
    exit 1
fi

echo -e "${GREEN}✓ HARMONYOS_NDK_PATH: $HARMONYOS_NDK_PATH${NC}"

# 检查 Rust 工具链
if ! command -v rustc &> /dev/null; then
    echo -e "${RED}错误: 未安装 Rust${NC}"
    echo "请访问 https://rustup.rs/ 安装 Rust"
    exit 1
fi

echo -e "${GREEN}✓ Rust 版本: $(rustc --version)${NC}"

# 添加鸿蒙目标
echo -e "\n${YELLOW}检查 Rust 交叉编译目标...${NC}"
if ! rustup target list --installed | grep -q "aarch64-linux-ohos"; then
    echo "添加鸿蒙目标: aarch64-linux-ohos"
    rustup target add aarch64-linux-ohos
fi
echo -e "${GREEN}✓ 鸿蒙目标已配置${NC}"

# 进入 Rust 模块目录
RUST_DIR="harmonyos/entry/ohos/rust"
cd "$RUST_DIR"

# 编译 Rust Native 模块
echo -e "\n${YELLOW}编译 Rust Native 模块...${NC}"
cargo build --target aarch64-linux-ohos --release

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Rust 模块编译成功${NC}"
else
    echo -e "${RED}✗ Rust 模块编译失败${NC}"
    exit 1
fi

# 复制 .so 文件到鸿蒙项目
echo -e "\n${YELLOW}复制 .so 文件...${NC}"
SO_FILE="target/aarch64-linux-ohos/release/libharmonydesk.so"
DEST_DIR="../../../libs/arm64-v8a"

mkdir -p "$DEST_DIR"
cp "$SO_FILE" "$DEST_DIR/"

echo -e "${GREEN}✓ .so 文件已复制到 $DEST_DIR${NC}"

# 返回项目根目录
cd - > /dev/null

echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}  Rust 模块构建完成！${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "\n${YELLOW}下一步:${NC}"
echo "1. 使用 DevEco Studio 打开项目"
echo "2. Build > Build App(s) / Hap(s) > Build Hap(s)"
echo "3. 在模拟器或真机上运行"
echo ""
echo -e "${YELLOW}提示:${NC}"
echo "- 首次构建需要在 DevEco Studio 中配置签名"
echo "- 真机调试需要在项目设置中配置包名和签名"
