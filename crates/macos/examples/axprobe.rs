//! AX API probe — discovers what every accessibility function returns on a live app.
//! Run: cargo run -p agent-desktop-macos --bin axprobe -- Finder
//!
//! This is a diagnostic tool used to learn exactly which attributes hold which
//! data before writing tree traversal code. Output is deliberately verbose.

fn main() {
    #[cfg(target_os = "macos")]
    run();

    #[cfg(not(target_os = "macos"))]
    eprintln!("axprobe only works on macOS");
}

#[cfg(target_os = "macos")]
fn run() {
    use accessibility_sys::*;
    use core_foundation::{
        array::CFArray,
        base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
        url::CFURL,
    };

    let app_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Finder".to_string());
    let pid = find_pid(&app_name).unwrap_or_else(|| {
        eprintln!("App '{}' not running", app_name);
        std::process::exit(1);
    });

    println!("=== axprobe: {} (pid {}) ===\n", app_name, pid);

    let app_el = unsafe { AXUIElementCreateApplication(pid) };
    unsafe { AXUIElementSetMessagingTimeout(app_el, 5.0) };

    // ── 1. App-level attributes ──────────────────────────────────────────
    println!("──────────────────────────────────────────────");
    println!("SECTION 1: App element attributes");
    println!("──────────────────────────────────────────────");
    dump_all_attrs(app_el, 0);

    // ── 2. Windows ───────────────────────────────────────────────────────
    println!("\n──────────────────────────────────────────────");
    println!("SECTION 2: Windows");
    println!("──────────────────────────────────────────────");
    let windows = copy_el_array(app_el, "AXWindows");
    println!("Window count: {}", windows.len());

    for (wi, &win) in windows.iter().enumerate() {
        unsafe { AXUIElementSetMessagingTimeout(win, 5.0) };
        let title = fetch_repr(win, "AXTitle");
        println!("\n  Window[{}] AXTitle={}", wi, title);
        dump_all_attrs(win, 2);

        // ── 3. Direct children of window ─────────────────────────────
        let children = copy_el_array(win, "AXChildren");
        println!("\n    Direct children: {}", children.len());

        for (ci, &child) in children.iter().enumerate() {
            let role = fetch_repr(child, "AXRole");
            let subrole = fetch_repr(child, "AXSubrole");
            let title = fetch_repr(child, "AXTitle");
            let desc = fetch_repr(child, "AXDescription");
            let value = fetch_repr(child, "AXValue");
            println!(
                "\n    Child[{}] role={} subrole={} title={} desc={} value={}",
                ci, role, subrole, title, desc, value
            );
            dump_all_attrs(child, 6);

            // Grandchildren
            let grandchildren = copy_el_array(child, "AXChildren");
            println!("      grandchildren: {}", grandchildren.len());
            for (gci, &gc) in grandchildren.iter().enumerate().take(10) {
                let r = fetch_repr(gc, "AXRole");
                let s = fetch_repr(gc, "AXSubrole");
                let t = fetch_repr(gc, "AXTitle");
                let d = fetch_repr(gc, "AXDescription");
                let v = fetch_repr(gc, "AXValue");
                println!(
                    "      GC[{}] role={} subrole={} title={} desc={} value={}",
                    gci, r, s, t, d, v
                );
                dump_all_attrs(gc, 8);

                // Great-grandchildren (just roles/names)
                let ggc = copy_el_array(gc, "AXChildren");
                for (ggci, &el) in ggc.iter().enumerate().take(6) {
                    let r2 = fetch_repr(el, "AXRole");
                    let t2 = fetch_repr(el, "AXTitle");
                    let d2 = fetch_repr(el, "AXDescription");
                    let v2 = fetch_repr(el, "AXValue");
                    println!(
                        "        GGC[{}] role={} title={} desc={} value={}",
                        ggci, r2, t2, d2, v2
                    );
                    dump_all_attrs(el, 10);

                    for &gggel in copy_el_array(el, "AXChildren").iter().take(4) {
                        let r3 = fetch_repr(gggel, "AXRole");
                        let t3 = fetch_repr(gggel, "AXTitle");
                        let v3 = fetch_repr(gggel, "AXValue");
                        println!("          GGGC role={} title={} value={}", r3, t3, v3);
                        unsafe { CFRelease(gggel as CFTypeRef) };
                    }
                    unsafe { CFRelease(el as CFTypeRef) };
                }
                unsafe { CFRelease(gc as CFTypeRef) };
            }
            unsafe { CFRelease(child as CFTypeRef) };
        }

        // Only probe first window in depth
        break;
    }

    // ── 4. AXCopyMultipleAttributeValues API test ────────────────────────
    println!("\n──────────────────────────────────────────────");
    println!("SECTION 3: AXUIElementCopyMultipleAttributeValues test");
    println!("──────────────────────────────────────────────");

    // Use first window child for the multi-attr test
    if !windows.is_empty() {
        let win = windows[0];
        let children = copy_el_array(win, "AXChildren");
        if !children.is_empty() {
            let el = children[0];
            let role = fetch_repr(el, "AXRole");
            println!("Test element: role={}", role);
            test_multi_attr(el);
            unsafe { CFRelease(el as CFTypeRef) };
        }
    }

    // ── 5. AXUIElementCopyElementAtPosition ─────────────────────────────
    println!("\n──────────────────────────────────────────────");
    println!("SECTION 4: AXUIElementCopyElementAtPosition");
    println!("──────────────────────────────────────────────");
    for (x, y) in [(400.0f32, 300.0), (600.0, 300.0), (200.0, 400.0)] {
        let mut pos_el: AXUIElementRef = std::ptr::null_mut();
        let err = unsafe { AXUIElementCopyElementAtPosition(app_el, x, y, &mut pos_el) };
        if err == 0 && !pos_el.is_null() {
            let r = fetch_repr(pos_el, "AXRole");
            let t = fetch_repr(pos_el, "AXTitle");
            let d = fetch_repr(pos_el, "AXDescription");
            let v = fetch_repr(pos_el, "AXValue");
            println!(
                "  ({},{}): role={} title={} desc={} value={}",
                x, y, r, t, d, v
            );
            unsafe { CFRelease(pos_el as CFTypeRef) };
        } else {
            println!("  ({},{}): err={}", x, y, err);
        }
    }

    // ── 6. Action names ──────────────────────────────────────────────────
    println!("\n──────────────────────────────────────────────");
    println!("SECTION 5: AXUIElementCopyActionNames");
    println!("──────────────────────────────────────────────");
    if !windows.is_empty() {
        let win = windows[0];
        let children = copy_el_array(win, "AXChildren");
        for &child in children.iter().take(5) {
            let role = fetch_repr(child, "AXRole");
            let actions = copy_action_names(child);
            println!("  {} => {:?}", role, actions);
            unsafe { CFRelease(child as CFTypeRef) };
        }
    }

    // Cleanup
    for &win in &windows {
        unsafe { CFRelease(win as CFTypeRef) };
    }
    unsafe { CFRelease(app_el as CFTypeRef) };
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn find_pid(app_name: &str) -> Option<i32> {
    let out = std::process::Command::new("pgrep")
        .arg("-x")
        .arg(app_name)
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
fn dump_all_attrs(el: accessibility_sys::AXUIElementRef, indent: usize) {
    use accessibility_sys::AXUIElementCopyAttributeNames;
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        string::CFString,
    };

    let pad = " ".repeat(indent);
    let mut names_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null_mut();
    let err = unsafe { AXUIElementCopyAttributeNames(el, &mut names_ref) };
    if err != 0 || names_ref.is_null() {
        println!("{}  <AXUIElementCopyAttributeNames err={}>", pad, err);
        return;
    }
    let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(names_ref as _) };
    let names: Vec<String> = arr
        .into_iter()
        .filter_map(|item| item.downcast::<CFString>().map(|s| s.to_string()))
        .collect();

    for name in &names {
        let val = fetch_repr(el, name);
        println!("{}  [attr] {}: {}", pad, name, val);
    }

    // Also test AXUIElementCopyParameterizedAttributeNames
    let mut pnames_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null_mut();
    let perr = unsafe {
        accessibility_sys::AXUIElementCopyParameterizedAttributeNames(el, &mut pnames_ref)
    };
    if perr == 0 && !pnames_ref.is_null() {
        let parr = unsafe { CFArray::<CFType>::wrap_under_create_rule(pnames_ref as _) };
        let pnames: Vec<String> = parr
            .into_iter()
            .filter_map(|item| item.downcast::<CFString>().map(|s| s.to_string()))
            .collect();
        if !pnames.is_empty() {
            println!("{}  [parameterized attrs]: {:?}", pad, pnames);
        }
    }
}

#[cfg(target_os = "macos")]
fn fetch_repr(el: accessibility_sys::AXUIElementRef, attr: &str) -> String {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
        url::CFURL,
    };

    let cf_attr = CFString::new(attr);
    let mut value: CFTypeRef = std::ptr::null_mut();
    let err =
        unsafe { AXUIElementCopyAttributeValue(el, cf_attr.as_concrete_TypeRef(), &mut value) };
    if err != 0 {
        return format!("<err:{}>", err);
    }
    if value.is_null() {
        return "<null>".to_string();
    }

    let cf = unsafe { CFType::wrap_under_create_rule(value) };

    if let Some(s) = cf.downcast::<CFString>() {
        return format!("\"{}\"", s.to_string());
    }
    if let Some(b) = cf.downcast::<CFBoolean>() {
        return format!("bool:{}", bool::from(b));
    }
    if let Some(n) = cf.downcast::<CFNumber>() {
        if let Some(i) = n.to_i64() {
            return format!("num:{}", i);
        }
        if let Some(f) = n.to_f64() {
            return format!("num:{:.2}", f);
        }
    }
    // Check if it's an array by type ID
    let arr_type_id = unsafe { core_foundation_sys::array::CFArrayGetTypeID() };
    if cf.type_of() == arr_type_id {
        let arr = unsafe {
            CFArray::<CFType>::wrap_under_get_rule(
                cf.as_concrete_TypeRef() as core_foundation_sys::array::CFArrayRef
            )
        };
        return format!("array[{}]", arr.len());
    }
    if let Some(url) = cf.downcast::<CFURL>() {
        return format!("url:{}", url.get_string().to_string());
    }

    format!("cftype:{}", cf.type_of())
}

#[cfg(target_os = "macos")]
fn copy_el_array(
    el: accessibility_sys::AXUIElementRef,
    attr: &str,
) -> Vec<accessibility_sys::AXUIElementRef> {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::{
        array::CFArray,
        base::{CFRetain, CFType, CFTypeRef, TCFType},
        string::CFString,
    };

    let cf_attr = CFString::new(attr);
    let mut value: CFTypeRef = std::ptr::null_mut();
    let err =
        unsafe { AXUIElementCopyAttributeValue(el, cf_attr.as_concrete_TypeRef(), &mut value) };
    if err != 0 || value.is_null() {
        return vec![];
    }
    let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(value as _) };
    arr.into_iter()
        .filter_map(|item| {
            let ptr = item.as_concrete_TypeRef() as accessibility_sys::AXUIElementRef;
            if ptr.is_null() {
                None
            } else {
                unsafe { CFRetain(ptr as CFTypeRef) };
                Some(ptr)
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn copy_action_names(el: accessibility_sys::AXUIElementRef) -> Vec<String> {
    use accessibility_sys::AXUIElementCopyActionNames;
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        string::CFString,
    };

    let mut ref_: core_foundation_sys::array::CFArrayRef = std::ptr::null_mut();
    let err = unsafe { AXUIElementCopyActionNames(el, &mut ref_) };
    if err != 0 || ref_.is_null() {
        return vec![];
    }
    let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(ref_ as _) };
    arr.into_iter()
        .filter_map(|item| item.downcast::<CFString>().map(|s| s.to_string()))
        .collect()
}

#[cfg(target_os = "macos")]
fn test_multi_attr(el: accessibility_sys::AXUIElementRef) {
    use accessibility_sys::{
        kAXCopyMultipleAttributeOptionStopOnError, AXUIElementCopyMultipleAttributeValues,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
    };

    let test_attrs = [
        "AXRole",
        "AXSubrole",
        "AXTitle",
        "AXDescription",
        "AXValue",
        "AXEnabled",
        "AXFocused",
        "AXHelp",
        "AXPlaceholderValue",
        "AXRoleDescription",
    ];

    for &options in &[0u32, kAXCopyMultipleAttributeOptionStopOnError] {
        let label = if options == 0 {
            "AllowPartial(0)"
        } else {
            "StopOnError(0x1)"
        };
        println!("  options={}", label);

        let cf_names: Vec<CFString> = test_attrs.iter().map(|a| CFString::new(a)).collect();
        let cf_refs: Vec<_> = cf_names.iter().map(|s| s.as_concrete_TypeRef()).collect();
        let names_arr = CFArray::from_copyable(&cf_refs);

        let mut result_ref: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyMultipleAttributeValues(
                el,
                names_arr.as_concrete_TypeRef(),
                options,
                &mut result_ref as *mut _ as *mut _,
            )
        };
        println!("    err={}", err);

        if err == 0 && !result_ref.is_null() {
            let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(result_ref as _) };
            println!(
                "    result_count={} (requested {})",
                arr.len(),
                test_attrs.len()
            );
            for (i, item) in arr.into_iter().enumerate() {
                let name = test_attrs.get(i).unwrap_or(&"?");
                let repr = if let Some(s) = item.downcast::<CFString>() {
                    format!("String(\"{}\")", s.to_string())
                } else if let Some(b) = item.downcast::<CFBoolean>() {
                    format!("Bool({})", bool::from(b))
                } else if let Some(n) = item.downcast::<CFNumber>() {
                    format!("Number({})", n.to_i64().unwrap_or(-1))
                } else {
                    format!("Other(type_id={})", item.type_of())
                };
                println!("    [{}] {} = {}", i, name, repr);
            }
        }
    }
}
