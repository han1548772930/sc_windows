use sc_drawing::Rect;
use sc_platform::WindowId;
use std::time::{Duration, Instant};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_BIND_UNORDERED_ACCESS, D3D11_BOX, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11CreateDevice,
    ID3D11ComputeShader, ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11UnorderedAccessView,
};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowRect};
use windows::core::{Interface, PCSTR, factory, s};

const WGC_FRAME_BUFFER_COUNT: i32 = 8;
#[derive(Clone, Debug)]
pub struct BgraFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub enum GpuFrameDecision {
    Skipped,
    Unmatched,
    Boundary,
    Keyframe {
        frame_id: u64,
        frame: BgraFrame,
        shift: i32,
        score: f32,
    },
}

pub struct GraphicsCaptureSource {
    frame_pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    staging: ID3D11Texture2D,
    gpu_anchor: ID3D11Texture2D,
    gpu_candidate: ID3D11Texture2D,
    anchor_srv: ID3D11ShaderResourceView,
    candidate_srv: ID3D11ShaderResourceView,
    score_texture: ID3D11Texture2D,
    score_staging: ID3D11Texture2D,
    score_uav: ID3D11UnorderedAccessView,
    motion_shader: ID3D11ComputeShader,
    gpu_anchor_ready: std::cell::Cell<bool>,
    gpu_candidate_pending: std::cell::Cell<bool>,
    gpu_pending_id: std::cell::Cell<Option<u64>>,
    gpu_next_id: std::cell::Cell<u64>,
    gpu_unmatched_count: std::cell::Cell<u32>,
    max_shift: i32,
    crop: D3D11_BOX,
    width: u32,
    height: u32,
}

impl GraphicsCaptureSource {
    pub fn new(target: WindowId, selection: Rect) -> Result<Self, String> {
        if !GraphicsCaptureSession::IsSupported().map_err(display_error)? {
            return Err("当前 Windows 版本不支持 Windows Graphics Capture".to_string());
        }

        let target = unsafe { GetAncestor(super::hwnd(target), GA_ROOT) };
        if target.0.is_null() {
            return Err("无法定位滚动截图目标窗口".to_string());
        }
        let mut window_rect = RECT::default();
        unsafe { GetWindowRect(target, &mut window_rect) }.map_err(display_error)?;

        let mut capture_rect = RECT::default();
        if unsafe {
            DwmGetWindowAttribute(
                target,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut capture_rect as *mut RECT).cast(),
                std::mem::size_of::<RECT>() as u32,
            )
        }
        .is_err()
        {
            capture_rect = window_rect;
        }

        let interop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(display_error)?;
        let item: GraphicsCaptureItem =
            unsafe { interop.CreateForWindow(target) }.map_err(display_error)?;
        let size = item.Size().map_err(display_error)?;

        let left = selection.left - capture_rect.left;
        let top = selection.top - capture_rect.top;
        let width = selection.right - selection.left;
        let height = selection.bottom - selection.top;
        if left < 0
            || top < 0
            || width <= 0
            || height <= 0
            || left + width > size.Width
            || top + height > size.Height
        {
            return Err(format!(
                "滚动截图选区不在目标窗口捕获范围内: 选区={}x{}@({}, {}), 窗口={}x{}",
                width, height, left, top, size.Width, size.Height
            ));
        }
        eprintln!(
            "[scroll capture] WGC crop={}x{}@({}, {}), capture={}x{}@({}, {}), texture={}x{}",
            width,
            height,
            left,
            top,
            capture_rect.right - capture_rect.left,
            capture_rect.bottom - capture_rect.top,
            capture_rect.left,
            capture_rect.top,
            size.Width,
            size.Height
        );

        let (device, context, winrt_device) = create_capture_device()?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            WGC_FRAME_BUFFER_COUNT,
            size,
        )
        .map_err(display_error)?;
        let session = frame_pool
            .CreateCaptureSession(&item)
            .map_err(display_error)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);

        let staging = create_staging_texture(&device, width as u32, height as u32)?;
        let gpu_anchor = create_gpu_frame_texture(&device, width as u32, height as u32)?;
        let gpu_candidate = create_gpu_frame_texture(&device, width as u32, height as u32)?;
        let anchor_srv = create_texture_srv(&device, &gpu_anchor)?;
        let candidate_srv = create_texture_srv(&device, &gpu_candidate)?;
        // A keyframe must overlap at least half of the previous frame. Smaller
        // overlap is too ambiguous on chat lists and other repeated layouts.
        let max_shift = (height as u32 / 2) as i32;
        let score_count = (max_shift * 2 + 1) as u32;
        let (score_texture, score_staging, score_uav) =
            create_score_textures(&device, score_count)?;
        let motion_shader = create_motion_shader(&device, width as u32, height as u32, max_shift)?;
        session.StartCapture().map_err(display_error)?;
        Ok(Self {
            frame_pool,
            session,
            _device: device,
            context,
            staging,
            gpu_anchor,
            gpu_candidate,
            anchor_srv,
            candidate_srv,
            score_texture,
            score_staging,
            score_uav,
            motion_shader,
            gpu_anchor_ready: std::cell::Cell::new(false),
            gpu_candidate_pending: std::cell::Cell::new(false),
            gpu_pending_id: std::cell::Cell::new(None),
            gpu_next_id: std::cell::Cell::new(1),
            gpu_unmatched_count: std::cell::Cell::new(0),
            max_shift,
            crop: D3D11_BOX {
                left: left as u32,
                top: top as u32,
                front: 0,
                right: (left + width) as u32,
                bottom: (top + height) as u32,
                back: 1,
            },
            width: width as u32,
            height: height as u32,
        })
    }

    pub fn try_next_frame(&self) -> Result<Option<BgraFrame>, String> {
        let mut latest = None;
        while let Ok(frame) = self.frame_pool.TryGetNextFrame() {
            latest = Some(frame);
        }
        latest.map(|frame| self.read_frame(&frame)).transpose()
    }

    pub fn try_next_gpu_frame(
        &self,
        direction: i8,
        force_keyframe: bool,
    ) -> Result<Option<GpuFrameDecision>, String> {
        if self.gpu_candidate_pending.get() {
            return Ok(None);
        }
        let Ok(frame) = self.frame_pool.TryGetNextFrame() else {
            return Ok(None);
        };
        let texture = frame_texture(&frame)?;
        self.validate_crop(&texture)?;
        self.copy_crop_to(&self.gpu_candidate, &texture);

        if !self.gpu_anchor_ready.get() {
            let pixels = self.read_texture(&self.gpu_candidate)?;
            unsafe {
                self.context
                    .CopyResource(&self.gpu_anchor, &self.gpu_candidate)
            };
            self.gpu_anchor_ready.set(true);
            return Ok(Some(GpuFrameDecision::Keyframe {
                frame_id: 0,
                frame: pixels,
                shift: 0,
                score: 0.0,
            }));
        }

        let scores = self.run_motion_shader()?;
        let raw_minimum_score = scores.iter().copied().min_by(f32::total_cmp).unwrap_or(1.0);
        let candidates: Vec<_> = scores
            .into_iter()
            .enumerate()
            .map(|(index, score)| (index as i32 - self.max_shift, score))
            .filter(|(shift, score)| {
                score.is_finite()
                    && *score < 1.0
                    && shift.unsigned_abs() <= self.max_shift as u32
                    && direction_allows_shift(direction, *shift)
            })
            .collect();
        let Some(best) = choose_best_shift(&candidates) else {
            let failures = self.gpu_unmatched_count.get().saturating_add(1);
            self.gpu_unmatched_count.set(failures);
            if failures == 1 || failures.is_multiple_of(30) {
                eprintln!(
                    "[scroll capture] GPU unmatched count={}, raw_min={:.5}, direction={}",
                    failures, raw_minimum_score, direction
                );
            }
            self.gpu_candidate_pending.set(false);
            return Ok(Some(GpuFrameDecision::Unmatched));
        };
        if has_equal_alternative(&candidates, best) {
            self.gpu_unmatched_count
                .set(self.gpu_unmatched_count.get().saturating_add(1));
            return Ok(Some(GpuFrameDecision::Unmatched));
        }
        if force_keyframe && best.0 == 0 && direction != 0 {
            unsafe {
                self.context
                    .CopyResource(&self.gpu_anchor, &self.gpu_candidate)
            };
            self.gpu_unmatched_count.set(0);
            return Ok(Some(GpuFrameDecision::Boundary));
        }
        if best.0 == 0 {
            unsafe {
                self.context
                    .CopyResource(&self.gpu_anchor, &self.gpu_candidate)
            };
            self.gpu_candidate_pending.set(false);
            return Ok(Some(GpuFrameDecision::Skipped));
        }
        let pixels = self.read_texture(&self.gpu_candidate)?;
        let frame_id = self.gpu_next_id.get();
        self.gpu_next_id.set(frame_id.wrapping_add(1).max(1));
        self.gpu_unmatched_count.set(0);
        self.gpu_candidate_pending.set(true);
        self.gpu_pending_id.set(Some(frame_id));
        Ok(Some(GpuFrameDecision::Keyframe {
            frame_id,
            frame: pixels,
            shift: best.0,
            score: best.1,
        }))
    }

    pub fn accept_gpu_candidate(&self, frame_id: u64) -> bool {
        if self.gpu_pending_id.get() == Some(frame_id) && self.gpu_candidate_pending.replace(false)
        {
            unsafe {
                self.context
                    .CopyResource(&self.gpu_anchor, &self.gpu_candidate)
            };
            self.gpu_pending_id.set(None);
            true
        } else {
            false
        }
    }

    pub fn discard_gpu_candidate(&self, frame_id: u64) -> bool {
        if self.gpu_pending_id.get() != Some(frame_id) {
            return false;
        }
        self.gpu_candidate_pending.set(false);
        self.gpu_pending_id.set(None);
        true
    }

    pub fn wait_for_first_frame(&self, timeout: Duration) -> Result<BgraFrame, String> {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if let Some(frame) = self.try_next_frame()? {
                return Ok(frame);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err("Windows Graphics Capture 启动后未收到首帧".to_string())
    }

    fn read_frame(&self, frame: &Direct3D11CaptureFrame) -> Result<BgraFrame, String> {
        let texture = frame_texture(frame)?;
        self.validate_crop(&texture)?;
        self.copy_crop_to(&self.gpu_candidate, &texture);
        let pixels = self.read_texture(&self.gpu_candidate)?;
        if !self.gpu_anchor_ready.replace(true) {
            unsafe {
                self.context
                    .CopyResource(&self.gpu_anchor, &self.gpu_candidate)
            };
        }
        Ok(pixels)
    }

    fn validate_crop(&self, texture: &ID3D11Texture2D) -> Result<(), String> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };
        if self.crop.right > desc.Width || self.crop.bottom > desc.Height {
            return Err("目标窗口尺寸在滚动截图期间发生变化".to_string());
        }

        Ok(())
    }

    fn copy_crop_to(&self, destination: &ID3D11Texture2D, source: &ID3D11Texture2D) {
        unsafe {
            self.context
                .CopySubresourceRegion(destination, 0, 0, 0, 0, source, 0, Some(&self.crop))
        };
    }

    fn read_texture(&self, texture: &ID3D11Texture2D) -> Result<BgraFrame, String> {
        unsafe { self.context.CopyResource(&self.staging, texture) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(display_error)?;

        let row_bytes = self.width as usize * 4;
        let mut pixels = vec![0; row_bytes * self.height as usize];
        for row in 0..self.height as usize {
            let source = unsafe {
                std::slice::from_raw_parts(
                    mapped
                        .pData
                        .cast::<u8>()
                        .add(row * mapped.RowPitch as usize),
                    row_bytes,
                )
            };
            pixels[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(source);
        }
        unsafe { self.context.Unmap(&self.staging, 0) };
        Ok(BgraFrame {
            width: self.width,
            height: self.height,
            pixels,
        })
    }

    fn run_motion_shader(&self) -> Result<Vec<f32>, String> {
        let resources = [
            Some(self.anchor_srv.clone()),
            Some(self.candidate_srv.clone()),
        ];
        let unordered = [Some(self.score_uav.clone())];
        unsafe {
            self.context.CSSetShader(&self.motion_shader, None);
            self.context.CSSetShaderResources(0, Some(&resources));
            self.context
                .CSSetUnorderedAccessViews(0, 1, Some(unordered.as_ptr()), None);
            let score_count = (self.max_shift * 2 + 1) as u32;
            self.context.Dispatch(score_count.div_ceil(64), 1, 1);
            self.context.CSSetShaderResources(0, Some(&[None, None]));
            let empty = [None];
            self.context
                .CSSetUnorderedAccessViews(0, 1, Some(empty.as_ptr()), None);
            self.context
                .CopyResource(&self.score_staging, &self.score_texture);
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&self.score_staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(display_error)?;
        let count = (self.max_shift * 2 + 1) as usize;
        let scores =
            unsafe { std::slice::from_raw_parts(mapped.pData.cast::<f32>(), count) }.to_vec();
        unsafe { self.context.Unmap(&self.score_staging, 0) };
        Ok(scores)
    }
}

fn choose_best_shift(candidates: &[(i32, f32)]) -> Option<(i32, f32)> {
    candidates
        .iter()
        .copied()
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.abs().cmp(&b.0.abs())))
}

fn has_equal_alternative(candidates: &[(i32, f32)], selected: (i32, f32)) -> bool {
    candidates
        .iter()
        .any(|candidate| candidate.0 != selected.0 && candidate.1 == selected.1)
}

fn direction_allows_shift(direction: i8, shift: i32) -> bool {
    match direction {
        1 => shift >= 0,
        -1 => shift <= 0,
        _ => shift == 0,
    }
}

impl Drop for GraphicsCaptureSource {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.frame_pool.Close();
    }
}

fn frame_texture(frame: &Direct3D11CaptureFrame) -> Result<ID3D11Texture2D, String> {
    let surface = frame.Surface().map_err(display_error)?;
    let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(display_error)?;
    unsafe { access.GetInterface() }.map_err(display_error)
}

fn create_capture_device() -> Result<(ID3D11Device, ID3D11DeviceContext, IDirect3DDevice), String> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .map_err(display_error)?;
    let device = device.ok_or_else(|| "D3D11 未返回捕获设备".to_string())?;
    let context = context.ok_or_else(|| "D3D11 未返回捕获上下文".to_string())?;
    let dxgi: IDXGIDevice = device.cast().map_err(display_error)?;
    let inspectable =
        unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }.map_err(display_error)?;
    let winrt_device: IDirect3DDevice = inspectable.cast().map_err(display_error)?;
    Ok((device, context, winrt_device))
}

fn create_staging_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<ID3D11Texture2D, String> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(display_error)?;
    texture.ok_or_else(|| "D3D11 未返回可读纹理".to_string())
}

fn create_gpu_frame_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
) -> Result<ID3D11Texture2D, String> {
    create_texture(
        device,
        width,
        height,
        DXGI_FORMAT_B8G8R8A8_UNORM,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_SHADER_RESOURCE.0 as u32,
        0,
    )
}

fn create_texture_srv(
    device: &ID3D11Device,
    texture: &ID3D11Texture2D,
) -> Result<ID3D11ShaderResourceView, String> {
    let mut view = None;
    unsafe { device.CreateShaderResourceView(texture, None, Some(&mut view)) }
        .map_err(display_error)?;
    view.ok_or_else(|| "D3D11 did not return a shader resource view".to_string())
}

fn create_score_textures(
    device: &ID3D11Device,
    count: u32,
) -> Result<(ID3D11Texture2D, ID3D11Texture2D, ID3D11UnorderedAccessView), String> {
    let score = create_texture(
        device,
        count,
        1,
        DXGI_FORMAT_R32_FLOAT,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_UNORDERED_ACCESS.0 as u32,
        0,
    )?;
    let staging = create_texture(
        device,
        count,
        1,
        DXGI_FORMAT_R32_FLOAT,
        D3D11_USAGE_STAGING,
        0,
        D3D11_CPU_ACCESS_READ.0 as u32,
    )?;
    let mut view = None;
    unsafe { device.CreateUnorderedAccessView(&score, None, Some(&mut view)) }
        .map_err(display_error)?;
    Ok((
        score,
        staging,
        view.ok_or_else(|| "D3D11 did not return a score UAV".to_string())?,
    ))
}

fn create_texture(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT,
    usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE,
    bind_flags: u32,
    cpu_access: u32,
) -> Result<ID3D11Texture2D, String> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: usage,
        BindFlags: bind_flags,
        CPUAccessFlags: cpu_access,
        MiscFlags: 0,
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(display_error)?;
    texture.ok_or_else(|| "D3D11 did not return a texture".to_string())
}

fn create_motion_shader(
    device: &ID3D11Device,
    width: u32,
    height: u32,
    max_shift: i32,
) -> Result<ID3D11ComputeShader, String> {
    let source = format!(
        r#"
Texture2D<float4> Anchor : register(t0);
Texture2D<float4> Candidate : register(t1);
RWTexture2D<float> Scores : register(u0);
static const int WIDTH = {width};
static const int HEIGHT = {height};
static const int MAX_SHIFT = {max_shift};

float luma(float4 pixel) {{ return dot(pixel.rgb, float3(0.114, 0.587, 0.299)); }}

[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {{
    int index = (int)id.x;
    if (index > MAX_SHIFT * 2) return;
    int shift = index - MAX_SHIFT;
    int y0 = max(1, 1 - shift);
    int y1 = min(HEIGHT - 1, HEIGHT - 1 - shift);
    float bandError[4] = {{ 0.0, 0.0, 0.0, 0.0 }};
    uint bandEvidence[4] = {{ 0, 0, 0, 0 }};
    for (int y = y0; y < y1; y += 2) {{
        int oldY = y + shift;
        int band = min(3, ((y - y0) * 4) / max(1, y1 - y0));
        for (int x = 2; x < WIDTH - 1; x += 4) {{
            float oldValue = luma(Anchor.Load(int3(x, oldY, 0)));
            float edge = max(abs(oldValue - luma(Anchor.Load(int3(x - 2, oldY, 0)))),
                             abs(oldValue - luma(Anchor.Load(int3(x, oldY - 1, 0)))));
            if (edge < 0.012) continue;
            float nextValue = luma(Candidate.Load(int3(x, y, 0)));
            bandError[band] += min(abs(oldValue - nextValue), 0.20);
            bandEvidence[band]++;
        }}
    }}
    float scoreSum = 0.0;
    float worstScore = 0.0;
    uint validBands = 0;
    for (int band = 0; band < 4; band++) {{
        if (bandEvidence[band] >= 16) {{
            float bandScore = bandError[band] / bandEvidence[band];
            scoreSum += bandScore;
            worstScore = max(worstScore, bandScore);
            validBands++;
        }}
    }}
    // A cursor, video, or loading animation may disturb one horizontal band.
    // Match on the consensus of the remaining bands instead of letting that
    // single dynamic region move an otherwise stationary frame.
    Scores[int2(index, 0)] = validBands >= 3
        ? (scoreSum - worstScore) / (validBands - 1)
        : 1.0;
}}
"#
    );
    let mut bytecode = None;
    let mut errors = None;
    let compiled = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None::<&windows::Win32::Graphics::Direct3D::ID3DInclude>,
            s!("main"),
            s!("cs_5_0"),
            0,
            0,
            &mut bytecode,
            Some(&mut errors),
        )
    };
    if let Err(error) = compiled {
        let details = errors.map_or_else(String::new, |blob| unsafe {
            let bytes = std::slice::from_raw_parts(
                blob.GetBufferPointer().cast::<u8>(),
                blob.GetBufferSize(),
            );
            String::from_utf8_lossy(bytes).into_owned()
        });
        return Err(format!(
            "GPU motion shader compilation failed: {error}; {details}"
        ));
    }
    let bytecode = bytecode.ok_or_else(|| "D3DCompile returned no shader".to_string())?;
    let bytes = unsafe {
        std::slice::from_raw_parts(
            bytecode.GetBufferPointer().cast::<u8>(),
            bytecode.GetBufferSize(),
        )
    };
    let mut shader = None;
    unsafe { device.CreateComputeShader(bytes, None, Some(&mut shader)) }.map_err(display_error)?;
    shader.ok_or_else(|| "D3D11 did not return a compute shader".to_string())
}

fn display_error(error: windows::core::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowest_error_shift_is_selected_without_prediction() {
        let candidates = [(-150, 0.020), (-90, 0.018), (-30, 0.017)];
        let selected = choose_best_shift(&candidates).unwrap();
        assert_eq!(selected.0, -30);
    }

    #[test]
    fn continuity_selection_does_not_require_a_global_photometric_threshold() {
        let candidates = [(-120, 0.19), (-82, 0.11), (-40, 0.18), (0, 0.20)];
        let selected = choose_best_shift(&candidates).unwrap();
        assert_eq!(selected.0, -82);
        assert!(!has_equal_alternative(&candidates, selected));
    }

    #[test]
    fn unchanged_boundary_frame_always_resolves_to_zero() {
        let candidates = [(0, 0.0), (60, 0.002), (120, 0.004)];
        let selected = choose_best_shift(&candidates).unwrap();
        assert_eq!(selected.0, 0);
    }

    #[test]
    fn lowest_dynamic_error_is_selected() {
        let candidates = [(0, 0.012), (60, 0.010), (120, 0.018)];
        let selected = choose_best_shift(&candidates).unwrap();
        assert_eq!(selected.0, 60);
    }

    #[test]
    fn wheel_direction_rejects_opposite_motion() {
        assert!(direction_allows_shift(-1, -67));
        assert!(!direction_allows_shift(-1, 26));
        assert!(direction_allows_shift(1, 75));
        assert!(!direction_allows_shift(1, -34));
        assert!(!direction_allows_shift(0, -34));
        assert!(direction_allows_shift(0, 0));
    }

    #[test]
    fn exactly_equal_scores_are_rejected_as_ambiguous() {
        let candidates = [(0, 0.02), (60, 0.001), (120, 0.001)];
        assert!(has_equal_alternative(&candidates, (60, 0.001)));
        assert!(!has_equal_alternative(
            &[(60, 0.001), (120, 0.0012)],
            (60, 0.001)
        ));
    }

    #[test]
    fn gpu_motion_pipeline_resources_are_supported() {
        let (device, _, _) = create_capture_device().expect("hardware D3D11 device");
        create_motion_shader(&device, 634, 407, 356).expect("compute shader");
        let (score, staging, uav) =
            create_score_textures(&device, 713).expect("GPU score resources");
        drop((score, staging, uav));
    }

    #[test]
    fn gpu_motion_shader_finds_known_vertical_shift() {
        const WIDTH: u32 = 320;
        const HEIGHT: u32 = 240;
        const SHIFT: i32 = 73;
        let (device, context, _) = create_capture_device().expect("hardware D3D11 device");
        let anchor = create_gpu_frame_texture(&device, WIDTH, HEIGHT).unwrap();
        let candidate = create_gpu_frame_texture(&device, WIDTH, HEIGHT).unwrap();
        let anchor_srv = create_texture_srv(&device, &anchor).unwrap();
        let candidate_srv = create_texture_srv(&device, &candidate).unwrap();
        let max_shift = (HEIGHT / 2) as i32;
        let score_count = (max_shift * 2 + 1) as u32;
        let (score, staging, uav) = create_score_textures(&device, score_count).unwrap();
        let shader = create_motion_shader(&device, WIDTH, HEIGHT, max_shift).unwrap();
        let document = |y_offset: u32| {
            let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let source_y = y + y_offset;
                    let hash = x
                        .wrapping_mul(37)
                        .wrapping_add(source_y.wrapping_mul(71))
                        .wrapping_add((x ^ source_y).wrapping_mul(13));
                    let index = ((y * WIDTH + x) * 4) as usize;
                    pixels[index] = (hash >> 5) as u8;
                    pixels[index + 1] = (hash >> 2) as u8;
                    pixels[index + 2] = hash as u8;
                    pixels[index + 3] = 255;
                }
            }
            pixels
        };
        let anchor_pixels = document(0);
        let candidate_pixels = document(SHIFT as u32);
        let started = Instant::now();
        unsafe {
            context.UpdateSubresource(
                &anchor,
                0,
                None,
                anchor_pixels.as_ptr().cast(),
                WIDTH * 4,
                0,
            );
            context.UpdateSubresource(
                &candidate,
                0,
                None,
                candidate_pixels.as_ptr().cast(),
                WIDTH * 4,
                0,
            );
            context.CSSetShader(&shader, None);
            context.CSSetShaderResources(0, Some(&[Some(anchor_srv), Some(candidate_srv)]));
            let unordered = [Some(uav)];
            context.CSSetUnorderedAccessViews(0, 1, Some(unordered.as_ptr()), None);
            for _ in 0..16 {
                context.Dispatch(score_count.div_ceil(64), 1, 1);
            }
            context.CopyResource(&staging, &score);
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }.unwrap();
        let scores =
            unsafe { std::slice::from_raw_parts(mapped.pData.cast::<f32>(), score_count as usize) };
        let best = scores
            .iter()
            .copied()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        unsafe { context.Unmap(&staging, 0) };
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "GPU motion dispatch performance regressed: {:?}",
            started.elapsed()
        );
        assert_eq!(best.0 as i32 - max_shift, SHIFT);
        assert!(best.1 <= 0.0001, "unexpected GPU overlap error: {}", best.1);

        let mut stationary_with_dynamic_band = anchor_pixels.clone();
        for y in 0..HEIGHT / 4 {
            for x in 0..WIDTH {
                let index = ((y * WIDTH + x) * 4) as usize;
                stationary_with_dynamic_band[index] ^= 0x7f;
                stationary_with_dynamic_band[index + 1] ^= 0x55;
                stationary_with_dynamic_band[index + 2] ^= 0x33;
            }
        }
        unsafe {
            context.UpdateSubresource(
                &candidate,
                0,
                None,
                stationary_with_dynamic_band.as_ptr().cast(),
                WIDTH * 4,
                0,
            );
            context.Dispatch(score_count.div_ceil(64), 1, 1);
            context.CopyResource(&staging, &score);
            context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .unwrap();
        let scores =
            unsafe { std::slice::from_raw_parts(mapped.pData.cast::<f32>(), score_count as usize) };
        let best = scores
            .iter()
            .copied()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        unsafe { context.Unmap(&staging, 0) };
        assert_eq!(best.0 as i32 - max_shift, 0);
    }
}
