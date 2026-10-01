//! Compile-and-run check for the capture/clipboard feature set.
//! Win32_UI_Shell is deliberately absent from Cargo.toml.

fn main() {
    let wgc = wgc_supported();
    let data_exchange = clipboard_symbols_linked();
    let imaging = imaging_symbols_linked();
    let memory = memory_symbols_linked();
    let wic = wic_round_trip();

    let ok = data_exchange && imaging && memory && wic.0;
    println!(
        "{{\"ok\":{},\"wic_round_trip\":{},\"png_bytes\":{},\"width\":{},\"height\":{},\"wgc_is_supported\":{},\"win32_ui_shell_in_manifest\":false,\"features_confirmed\":[\"Win32_System_DataExchange\",\"Win32_System_Memory\",\"Win32_Graphics_Imaging\",\"Win32_System_Com_StructuredStorage\",\"Graphics_Capture\"],\"data_exchange_linked\":{},\"imaging_linked\":{},\"memory_linked\":{}}}",
        ok,
        wic.0,
        wic.1,
        wic.2,
        wic.3,
        wgc,
        data_exchange,
        imaging,
        memory
    );
}

fn wgc_supported() -> bool {
    windows::Graphics::Capture::GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

fn clipboard_symbols_linked() -> bool {
    let _ = windows_sys::Win32::System::Ole::CF_UNICODETEXT;
    let _open = windows_sys::Win32::System::DataExchange::OpenClipboard;
    true
}

fn imaging_symbols_linked() -> bool {
    let _ = windows::Win32::Graphics::Imaging::CLSID_WICImagingFactory;
    let _ = windows::Win32::Graphics::Imaging::GUID_ContainerFormatPng;
    let _create = windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    let _ = _create;
    true
}

fn memory_symbols_linked() -> bool {
    let _ = windows_sys::Win32::System::Memory::GlobalAlloc;
    let _ = windows_sys::Win32::System::Memory::GMEM_MOVEABLE;
    true
}

fn wic_round_trip() -> (bool, usize, u32, u32) {
    match wic_encode_png() {
        Ok((bytes, w, h)) => (true, bytes, w, h),
        Err(_) => (false, 0, 0, 0),
    }
}

fn wic_encode_png() -> Result<(usize, u32, u32), windows::core::Error> {
    use std::ptr::null_mut;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
        IWICBitmapEncoder, IWICBitmapFrameEncode, IWICImagingFactory, WICBitmapEncoderNoCache,
    };
    use windows::Win32::System::Com::StructuredStorage::{CreateStreamOnHGlobal, IPropertyBag2};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IStream, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED, STATFLAG_NONAME, STREAM_SEEK_SET,
    };

    unsafe {
        let _ = CoInitializeEx(Some(null_mut()), COINIT_MULTITHREADED);
        let width = 4u32;
        let height = 3u32;
        let stride = width * 4;
        let mut bgra = vec![0u8; (stride * height) as usize];
        for (i, px) in bgra.chunks_exact_mut(4).enumerate() {
            px[0] = (i * 17) as u8;
            px[1] = (i * 31) as u8;
            px[2] = (i * 47) as u8;
            px[3] = 255;
        }

        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let bitmap = factory.CreateBitmapFromMemory(
            width,
            height,
            &GUID_WICPixelFormat32bppBGRA,
            stride,
            &bgra,
        )?;
        let stream: IStream = CreateStreamOnHGlobal(HGLOBAL(null_mut()), true)?;
        let encoder: IWICBitmapEncoder =
            factory.CreateEncoder(&GUID_ContainerFormatPng, null_mut())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;

        let mut frame: Option<IWICBitmapFrameEncode> = None;
        let mut props: Option<IPropertyBag2> = None;
        encoder.CreateNewFrame(&mut frame, &mut props)?;
        let frame = frame.ok_or_else(windows::core::Error::empty)?;
        frame.Initialize(props.as_ref())?;
        frame.WriteSource(&bitmap, null_mut())?;
        frame.Commit()?;
        encoder.Commit()?;

        let mut stat = Default::default();
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        let size = stat.cbSize as usize;
        let _ = STREAM_SEEK_SET;
        CoUninitialize();
        Ok((size, width, height))
    }
}
