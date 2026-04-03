#!/usr/bin/env bash
set -euo pipefail

REPO="weijunjiang123/skill-repo-rs"
BINARY="skill-repo"
INSTALL_DIR="${HOME}/.local/bin"

# 检测平台
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)  TARGET="x86_64-unknown-linux-gnu" ;;
  Darwin)
    case "${ARCH}" in
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      *)             TARGET="x86_64-apple-darwin" ;;
    esac
    ;;
  *) echo "不支持的操作系统: ${OS}"; exit 1 ;;
esac

echo "检测到平台: ${TARGET}"

# 直接用 GitHub latest 重定向下载，无需 API 调用，不受限流影响
URL="https://github.com/${REPO}/releases/latest/download/${BINARY}-${TARGET}.tar.gz"
echo "下载: ${URL}"

TMP=$(mktemp -d)
trap 'rm -rf "${TMP}"' EXIT

curl -fsSL "${URL}" -o "${TMP}/archive.tar.gz"
tar xzf "${TMP}/archive.tar.gz" -C "${TMP}"

# 安装
mkdir -p "${INSTALL_DIR}"
mv "${TMP}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

echo ""
echo "已安装到 ${INSTALL_DIR}/${BINARY}"

# 检查 PATH
if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
  echo ""
  echo "请将以下内容添加到 ~/.bashrc 或 ~/.zshrc:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo ""
echo "运行 'skill-repo --help' 开始使用"
