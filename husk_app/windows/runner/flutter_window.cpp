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
    KillTimer(GetHandle(), kHuskRenderTimerId);
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
    case WM_TIMER:
      if (wparam == kHuskRenderTimerId && husk_renderer_) {
        husk_bridge_.RenderFrame(husk_renderer_);
        husk_texture_registrar_->MarkTextureFrameAvailable(husk_texture_id_);
      }
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

  // Everything that needs the graphics runs inside this one-pass loop, so a
  // failure can leave it with `break` and still reach the method channel
  // below. Dart then hears why there is no viewport, instead of waiting for
  // a texture that will never come.
  do {
  if (!husk_bridge_.Load()) {
    OutputDebugStringA("Husk: failed to load husk_core.dll\n");
    husk_error_ =
        "Husk's graphics library (husk_core.dll) could not be loaded.";
    break;
  }
  const uint32_t width = 1000;
  const uint32_t height = 1000;
  husk_renderer_ = husk_bridge_.CreateRenderer(width, height);
  if (!husk_renderer_) {
    OutputDebugStringA("Husk: the graphics could not be started\n");
    husk_error_ =
        "The graphics could not be started. No usable GPU or graphics "
        "driver was found.";
    break;
  }
  husk_width_ = width;
  husk_height_ = height;
  // The bridge already loaded the DLL, so this only finds its resize function.
  husk_resize_ = reinterpret_cast<HuskResizeFn>(GetProcAddress(
      GetModuleHandleW(L"husk_core.dll"), "husk_renderer_resize"));
  husk_bridge_.RenderFrame(husk_renderer_);

  // ~30 FPS test loop. Will be replaced with a proper render/vsync loop
  // once real scene content exists.
  SetTimer(GetHandle(), kHuskRenderTimerId, 33, nullptr);

  husk_texture_ = std::make_unique<flutter::TextureVariant>(

flutter::PixelBufferTexture(
          [this](size_t w, size_t h) -> const FlutterDesktopPixelBuffer* {
            return CopyHuskPixelBuffer(w, h);
          }));

  husk_texture_id_ =
      husk_texture_registrar_->RegisterTexture(husk_texture_.get());
  } while (false);
  husk_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(), "husk/texture",
          &flutter::StandardMethodCodec::GetInstance());
  husk_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        if (call.method_name() == "getTextureId") {
          if (!husk_error_.empty()) {
            result->Error("graphics_unavailable", husk_error_);
          } else {
            result->Success(flutter::EncodableValue(husk_texture_id_));
          }
        } else if (call.method_name() == "resize") {
          int32_t new_width = 0;
          int32_t new_height = 0;
          const auto* arguments =
              std::get_if<flutter::EncodableMap>(call.arguments());
          if (arguments) {
            auto width_entry =
                arguments->find(flutter::EncodableValue("width"));
            auto height_entry =
                arguments->find(flutter::EncodableValue("height"));
            if (width_entry != arguments->end()) {
              if (const auto* value =
                      std::get_if<int32_t>(&width_entry->second)) {
                new_width = *value;
              }
            }
            if (height_entry != arguments->end()) {
              if (const auto* value =
                      std::get_if<int32_t>(&height_entry->second)) {
                new_height = *value;
              }
            }
          }
          if (new_width < 16 || new_width > 4096 || new_height < 16 ||
              new_height > 4096 || !husk_renderer_ || !husk_resize_) {
            result->Error("bad_resize",
                          "Resize needs a width and height between 16 and 4096");
          } else {
            if (ResizeHuskRenderer(static_cast<uint32_t>(new_width),
                                   static_cast<uint32_t>(new_height))) {
              result->Success();
            } else {
              result->Error(
                  "resize_failed",
                  "The viewport could not be resized; it keeps its old size");
            }
          }
        } else {
                  result->NotImplemented();
        }
      });
}

const FlutterDesktopPixelBuffer* FlutterWindow::CopyHuskPixelBuffer(
    size_t width, size_t height) {
  // Read the frame's pointer and size together, so a resize on the other
  // thread can never leave them out of step.
  std::lock_guard<std::mutex> lock(husk_frame_mutex_);
  // No renderer, or no frame drawn yet: there is nothing to show.
  if (!husk_renderer_) {
    return nullptr;
  }
  const auto* frame = husk_bridge_.FramePtr(husk_renderer_);
  if (!frame) {
    return nullptr;
  }
  husk_pixel_buffer_descriptor_.buffer = frame;
  husk_pixel_buffer_descriptor_.width = husk_width_;
  husk_pixel_buffer_descriptor_.height = husk_height_;  
  husk_pixel_buffer_descriptor_.release_callback = nullptr;
  husk_pixel_buffer_descriptor_.release_context = nullptr;
  return &husk_pixel_buffer_descriptor_;
}

bool FlutterWindow::ResizeHuskRenderer(uint32_t width, uint32_t height) {
// Held while the size changes so the engine never reads a frame together
  // with the wrong size.
  std::lock_guard<std::mutex> lock(husk_frame_mutex_);
  // If the renderer could not change size it keeps its old one, and so must
  // the size the engine reads frames at.
  if (!husk_resize_(husk_renderer_, width, height)) {
    return false;
  }
  husk_width_ = width;
  husk_height_ = height;
  return true;
}