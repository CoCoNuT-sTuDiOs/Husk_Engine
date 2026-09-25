#include "husk_bridge.h"

#include <windows.h>

bool HuskBridge::Load() {
  HMODULE module = LoadLibraryW(L"husk_core.dll");
  if (!module) {
    return false;
  }

  create_renderer_ = reinterpret_cast<CreateRendererFn>(
      GetProcAddress(module, "husk_renderer_create"));
  render_frame_ = reinterpret_cast<RenderFrameFn>(
      GetProcAddress(module, "husk_renderer_render_frame"));
  frame_ptr_ = reinterpret_cast<FramePtrFn>(
      GetProcAddress(module, "husk_renderer_frame_ptr"));
  frame_len_ = reinterpret_cast<FrameLenFn>(
      GetProcAddress(module, "husk_renderer_frame_len"));
  destroy_renderer_ = reinterpret_cast<DestroyRendererFn>(
      GetProcAddress(module, "husk_renderer_destroy"));

  return create_renderer_ && render_frame_ && frame_ptr_ && frame_len_ &&
         destroy_renderer_;
}

void* HuskBridge::CreateRenderer(uint32_t width, uint32_t height) {
  return create_renderer_ ? create_renderer_(width, height) : nullptr;
}

void HuskBridge::RenderFrame(void* handle) {
  if (render_frame_) {
    render_frame_(handle);
  }
}

const uint8_t* HuskBridge::FramePtr(void* handle) {
  return frame_ptr_ ? frame_ptr_(handle) : nullptr;
}

size_t HuskBridge::FrameLen(void* handle) {
  return frame_len_ ? frame_len_(handle) : 0;
}

void HuskBridge::DestroyRenderer(void* handle) {
  if (destroy_renderer_) {
    destroy_renderer_(handle);
  }
}