#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <flutter/texture_registrar.h>

#include <cstdint>
#include <memory>
#include <vector>

#include "husk_bridge.h"
#include "win32_window.h"

// A window that does nothing but host a Flutter view.
class FlutterWindow : public Win32Window {
 public:
  // Creates a new FlutterWindow hosting a Flutter view running |project|.
  explicit FlutterWindow(const flutter::DartProject& project);
  virtual ~FlutterWindow();

 protected:
  // Win32Window:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  // The project to run.
  flutter::DartProject project_;

  // The Flutter instance hosted by this window.
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;

  // Husk's render output texture, now backed by a real wgpu render.
  flutter::TextureRegistrar* husk_texture_registrar_ = nullptr;
  std::unique_ptr<flutter::TextureVariant> husk_texture_;
  int64_t husk_texture_id_ = -1;
  FlutterDesktopPixelBuffer husk_pixel_buffer_descriptor_ = {};

  HuskBridge husk_bridge_;
  void* husk_renderer_ = nullptr;
  static constexpr UINT_PTR kHuskRenderTimerId = 1;
  
  // Method channel used to hand the texture ID to Dart.
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>>
      husk_channel_;

  // Builds a solid-color test frame and registers it as a texture.
  void SetUpHuskTexture();

  // Callback invoked by the engine to fetch the latest frame.
  const FlutterDesktopPixelBuffer* CopyHuskPixelBuffer(size_t width,
                                                       size_t height);
};
#endif  // RUNNER_FLUTTER_WINDOW_H_
