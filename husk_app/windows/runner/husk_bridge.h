#ifndef RUNNER_HUSK_BRIDGE_H_
#define RUNNER_HUSK_BRIDGE_H_

#include <cstddef>
#include <cstdint>

// Thin wrapper that dynamically loads husk_core.dll at runtime and exposes
// its renderer FFI functions with normal C++ call syntax. Loading it this
// way avoids needing to statically link against cargokit's build output.
class HuskBridge {
 public:
  // Loads husk_core.dll and resolves all needed function pointers.
  // Returns false if the DLL or any symbol can't be found.
  bool Load();

  // Creates a renderer of the given size. Returns an opaque handle, or
  // nullptr on failure.
  void* CreateRenderer(uint32_t width, uint32_t height);

  // Renders the next frame into the renderer's internal buffer.
  void RenderFrame(void* handle);

  // Returns a pointer to the most recently rendered frame's pixel data.
  const uint8_t* FramePtr(void* handle);

  // Returns the length in bytes of the most recently rendered frame.
  size_t FrameLen(void* handle);

  // Destroys a renderer created with CreateRenderer.
  void DestroyRenderer(void* handle);

 private:
  typedef void* (*CreateRendererFn)(uint32_t, uint32_t);
  typedef void (*RenderFrameFn)(void*);
  typedef const uint8_t* (*FramePtrFn)(void*);
  typedef size_t (*FrameLenFn)(void*);
  typedef void (*DestroyRendererFn)(void*);

  CreateRendererFn create_renderer_ = nullptr;
  RenderFrameFn render_frame_ = nullptr;
  FramePtrFn frame_ptr_ = nullptr;
  FrameLenFn frame_len_ = nullptr;
  DestroyRendererFn destroy_renderer_ = nullptr;
};

#endif  // RUNNER_HUSK_BRIDGE_H_