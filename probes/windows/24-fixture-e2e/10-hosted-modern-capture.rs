use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HMODULE, POINT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{HMONITOR, MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::Interface;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;

const PROBE_NAME: &str = "24-fixture-e2e-10-hosted-modern-capture";
const QUESTION: &str = "does a real Windows.Graphics.Capture frame capture against this host's primary monitor produce non-degenerate pixels, and which branch fires (unsupported, supported-and-succeeded, supported-and-failed)";
const FRAME_TIMEOUT: Duration = Duration::from_secs(8);
const FRAME_POLL: Duration = Duration::from_millis(20);

struct CaptureStats {
    width: u32,
    height: u32,
    sampled_pixel_count: u64,
    nonzero_pixel_count: u64,
    appears_black: bool,
}

struct CaptureDevice {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
}

impl CaptureDevice {
    fn create() -> Result<Self, String> {
        unsafe {
            let mut device = None;
            let mut context = None;
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
            .map_err(|e| format!("D3D11CreateDevice failed: {:?}", e.code()))?;
            let device = device.ok_or("D3D11CreateDevice returned no device")?;
            let context = context.ok_or("D3D11CreateDevice returned no device context")?;
            let dxgi: IDXGIDevice = device
                .cast()
                .map_err(|e| format!("cast to IDXGIDevice failed: {:?}", e.code()))?;
            let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice failed: {:?}", e.code()))?;
            let winrt: IDirect3DDevice = inspectable
                .cast()
                .map_err(|e| format!("cast to IDirect3DDevice failed: {:?}", e.code()))?;
            Ok(Self {
                device,
                context,
                winrt,
            })
        }
    }
}

fn primary_monitor() -> Result<HMONITOR, String> {
    let point = POINT { x: 0, y: 0 };
    let handle = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTOPRIMARY) };
    if handle.0.is_null() {
        return Err("MonitorFromPoint returned a null handle".to_string());
    }
    Ok(handle)
}

fn item_for_monitor(monitor: HMONITOR) -> Result<GraphicsCaptureItem, String> {
    let interop: IGraphicsCaptureItemInterop =
        windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|e| format!("activate IGraphicsCaptureItemInterop failed: {:?}", e.code()))?;
    unsafe {
        interop
            .CreateForMonitor::<GraphicsCaptureItem>(monitor)
            .map_err(|e| format!("CreateForMonitor failed: {:?}", e.code()))
    }
}

fn texture_from_surface(
    surface: &windows::Graphics::DirectX::Direct3D11::IDirect3DSurface,
) -> Result<ID3D11Texture2D, String> {
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|e| format!("access the frame surface DXGI interface failed: {:?}", e.code()))?;
    unsafe {
        access
            .GetInterface::<ID3D11Texture2D>()
            .map_err(|e| format!("GetInterface<ID3D11Texture2D> failed: {:?}", e.code()))
    }
}

fn read_texture_bgra(
    device: &CaptureDevice,
    texture: &ID3D11Texture2D,
    content_width: u32,
    content_height: u32,
) -> Result<Vec<u8>, String> {
    unsafe {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        let width = content_width.min(desc.Width);
        let height = content_height.min(desc.Height);
        if width == 0 || height == 0 {
            return Err("modern capture produced a zero-sized frame".to_string());
        }
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        device
            .device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .map_err(|e| format!("CreateTexture2D failed: {:?}", e.code()))?;
        let staging = staging.ok_or("CreateTexture2D returned no staging texture")?;
        device.context.CopyResource(&staging, texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        device
            .context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| format!("Map failed: {:?}", e.code()))?;
        let pixels = copy_mapped_bgra(&mapped, width, height);
        device.context.Unmap(&staging, 0);
        pixels
    }
}

fn copy_mapped_bgra(
    mapped: &D3D11_MAPPED_SUBRESOURCE,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    if mapped.pData.is_null() {
        return Err("mapped staging texture had a null data pointer".to_string());
    }
    let row_pitch = mapped.RowPitch as usize;
    let width = width as usize;
    let height = height as usize;
    let row_bytes = width * 4;
    if row_pitch < row_bytes {
        return Err("mapped staging texture row pitch is shorter than the content stride".to_string());
    }
    let mut out = vec![0u8; row_bytes * height];
    unsafe {
        let src = mapped.pData as *const u8;
        for y in 0..height {
            let src_row = src.add(y * row_pitch);
            let dst_off = y * row_bytes;
            std::ptr::copy_nonoverlapping(src_row, out[dst_off..].as_mut_ptr(), row_bytes);
        }
    }
    Ok(out)
}

fn attempt_capture() -> Result<CaptureStats, String> {
    let monitor = primary_monitor()?;
    let item = item_for_monitor(monitor)?;
    let size = item
        .Size()
        .map_err(|e| format!("GraphicsCaptureItem.Size failed: {:?}", e.code()))?;
    if size.Width <= 0 || size.Height <= 0 {
        return Err("capture item reported a zero-sized frame".to_string());
    }

    let device = CaptureDevice::create()?;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device.winrt,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        size,
    )
    .map_err(|e| format!("CreateFreeThreaded failed: {:?}", e.code()))?;
    let session = pool
        .CreateCaptureSession(&item)
        .map_err(|e| format!("CreateCaptureSession failed: {:?}", e.code()))?;
    let _ = session.SetIsCursorCaptureEnabled(false);
    let _ = session.SetIsBorderRequired(false);
    session
        .StartCapture()
        .map_err(|e| format!("StartCapture failed: {:?}", e.code()))?;

    let deadline = Instant::now() + FRAME_TIMEOUT;
    let frame = loop {
        match pool.TryGetNextFrame() {
            Ok(frame) => break frame,
            Err(_) => {
                if Instant::now() >= deadline {
                    let _ = session.Close();
                    let _ = pool.Close();
                    return Err(format!(
                        "no frame arrived within {}ms",
                        FRAME_TIMEOUT.as_millis()
                    ));
                }
                std::thread::sleep(FRAME_POLL);
            }
        }
    };

    let content = frame
        .ContentSize()
        .map_err(|e| format!("ContentSize failed: {:?}", e.code()))?;
    let width = content.Width.max(0) as u32;
    let height = content.Height.max(0) as u32;
    let surface = frame
        .Surface()
        .map_err(|e| format!("Surface failed: {:?}", e.code()))?;
    let texture = texture_from_surface(&surface)?;
    let pixels = read_texture_bgra(&device, &texture, width, height);
    let _ = frame.Close();
    let _ = session.Close();
    let _ = pool.Close();
    let pixels = pixels?;

    let mut nonzero: u64 = 0;
    let mut index = 0usize;
    while index + 4 <= pixels.len() {
        if pixels[index] != 0 || pixels[index + 1] != 0 || pixels[index + 2] != 0 {
            nonzero += 1;
        }
        index += 4;
    }
    let sampled = (pixels.len() / 4) as u64;
    Ok(CaptureStats {
        width,
        height,
        sampled_pixel_count: sampled,
        nonzero_pixel_count: nonzero,
        appears_black: nonzero == 0,
    })
}

fn run() -> serde_json::Value {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let supported = GraphicsCaptureSession::IsSupported().unwrap_or(false);
    if !supported {
        return serde_json::json!({
            "probe": PROBE_NAME,
            "question": QUESTION,
            "measurable": false,
            "branch": "unsupported_on_host",
            "wgc_is_supported": false,
        });
    }

    let started = Instant::now();
    match attempt_capture() {
        Ok(stats) => serde_json::json!({
            "probe": PROBE_NAME,
            "question": QUESTION,
            "measurable": true,
            "branch": "supported_capture_succeeded",
            "wgc_is_supported": true,
            "width": stats.width,
            "height": stats.height,
            "sampled_pixel_count": stats.sampled_pixel_count,
            "nonzero_pixel_count": stats.nonzero_pixel_count,
            "appears_black": stats.appears_black,
            "elapsed_ms": started.elapsed().as_millis() as u64,
        }),
        Err(reason) => serde_json::json!({
            "probe": PROBE_NAME,
            "question": QUESTION,
            "measurable": false,
            "branch": "supported_capture_failed",
            "wgc_is_supported": true,
            "failure_reason": reason,
            "elapsed_ms": started.elapsed().as_millis() as u64,
        }),
    }
}

fn main() {
    let value = run();
    match serde_json::to_string(&value) {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("10-hosted-modern-capture: failed to serialize result: {error}");
            std::process::exit(1);
        }
    }
}
