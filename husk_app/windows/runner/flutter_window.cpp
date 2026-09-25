#include "flutter_window.h"

#include <optional>

#include "flutter/generated_plugin_registrant.h"

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  SetUpHuskTexture();
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  if (husk_renderer_) {
    husk_bridge_.DestroyRenderer(husk_renderer_);
    husk_renderer_ = nullptr;
  }

  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}


void FlutterWindow::SetUpHuskTexture() {
  // Borrow a plugin registrar under an arbitrary, unique name — Husk isn't a
  // real Dart plugin, just reusing the same path plugins use internally to
  // reach the texture registrar.
  FlutterDesktopPluginRegistrarRef core_registrar =
      flutter_controller_->engine()->GetRegistrarForPlugin("HuskEngineTexture");
  auto* registrar =
      flutter::PluginRegistrarManager::GetInstance()
          ->GetRegistrar<flutter::PluginRegistrarWindows>(core_registrar);
  husk_texture_registrar_ = registrar->texture_registrar();

  if (!husk_bridge_.Load()) {
    OutputDebugStringA("Husk: failed to load husk_core.dll\n");
    return;
  }

  const uint32_t width = 256;
  const uint32_t height = 256;
  husk_renderer_ = husk_bridge_.CreateRenderer(width, height);
  husk_bridge_.RenderFrame(husk_renderer_);

  husk_texture_ = std::make_unique<flutter::TextureVariant>(

flutter::PixelBufferTexture(
          [this](size_t w, size_t h) -> const FlutterDesktopPixelBuffer* {
            return CopyHuskPixelBuffer(w, h);
          }));

  husk_texture_id_ =
      husk_texture_registrar_->RegisterTexture(husk_texture_.get());

  husk_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(), "husk/texture",
          &flutter::StandardMethodCodec::GetInstance());
  husk_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        if (call.method_name() == "getTextureId") {
          result->Success(flutter::EncodableValue(husk_texture_id_));
        } else {
          result->NotImplemented();
        }
      });
}

const FlutterDesktopPixelBuffer* FlutterWindow::CopyHuskPixelBuffer(
    size_t width, size_t height) {
  husk_pixel_buffer_descriptor_.buffer =
      husk_bridge_.FramePtr(husk_renderer_);
  husk_pixel_buffer_descriptor_.width = 256;
  husk_pixel_buffer_descriptor_.height = 256;
  husk_pixel_buffer_descriptor_.release_callback = nullptr;
  husk_pixel_buffer_descriptor_.release_context = nullptr;
  return &husk_pixel_buffer_descriptor_;
}