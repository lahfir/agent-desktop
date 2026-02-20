//! Deep probe of AXOutline sidebar rows and AXBrowser columns in Finder.
//! Run: cargo run -p agent-desktop-macos --bin axprobe2

fn main() {
    #[cfg(target_os = "macos")]
    run();
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS only");
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
    };

    let pid = find_pid("Finder").unwrap_or_else(|| {
        eprintln!("Finder not running");
        std::process::exit(1);
    });

    let app_el = unsafe { AXUIElementCreateApplication(pid) };
    unsafe { AXUIElementSetMessagingTimeout(app_el, 5.0) };

    let windows = copy_el_array(app_el, "AXWindows");
    let win = match windows.first() {
        Some(&w) => w,
        None => {
            eprintln!("No windows");
            return;
        }
    };
    unsafe { AXUIElementSetMessagingTimeout(win, 5.0) };

    println!("=== Finder Window: {} ===\n", fetch_str(win, "AXTitle"));

    // ── Sidebar AXOutline → AXRow children ─────────────────────────────
    println!("────────────────────────────────────────");
    println!("PART 1: Sidebar AXOutline rows (6 levels deep)");
    println!("────────────────────────────────────────");

    let sidebar_outline = find_by_desc(win, "sidebar", 0);
    if let Some(outline) = sidebar_outline {
        let outline_role = fetch_str(outline, "AXRole");
        let outline_desc = fetch_str(outline, "AXDescription");
        println!("Found: role={} desc={}", outline_role, outline_desc);
        println!("AXRows count: {}", count_attr(outline, "AXRows"));
        println!("AXChildren count: {}", count_attr(outline, "AXChildren"));

        let rows = copy_el_array(outline, "AXRows");
        println!("Probing first 8 AXRows (all attributes + full children):");
        for (i, &row) in rows.iter().enumerate().take(8) {
            dump_element(row, i, 2);
        }
        for &row in &rows {
            unsafe { CFRelease(row as CFTypeRef) };
        }
        unsafe { CFRelease(outline as CFTypeRef) };
    } else {
        println!("Could not find sidebar outline");
    }

    // ── AXBrowser column view ───────────────────────────────────────────
    println!("\n────────────────────────────────────────");
    println!("PART 2: AXBrowser column view (AXColumns)");
    println!("────────────────────────────────────────");

    let browser = find_by_role(win, "AXBrowser", 0);
    if let Some(br) = browser {
        println!("Found AXBrowser: desc={}", fetch_str(br, "AXDescription"));
        println!("AXColumns: {}", count_attr(br, "AXColumns"));
        println!("AXChildren: {}", count_attr(br, "AXChildren"));
        println!("AXVisibleColumns: {}", count_attr(br, "AXVisibleColumns"));

        // Probe each column
        let columns = copy_el_array(br, "AXColumns");
        println!("\nProbing AXColumns:");
        for (ci, &col) in columns.iter().enumerate() {
            println!("\n  Column[{}]:", ci);
            dump_all_attrs(col, 4);
            let col_rows = copy_el_array(col, "AXRows");
            let col_children = copy_el_array(col, "AXChildren");
            println!(
                "    AXRows={} AXChildren={}",
                col_rows.len(),
                col_children.len()
            );

            // Probe first few rows
            for (ri, &row) in col_rows.iter().enumerate().take(6) {
                dump_element(row, ri, 6);
            }
            for &row in &col_rows {
                unsafe { CFRelease(row as CFTypeRef) };
            }
            for &child in &col_children {
                unsafe { CFRelease(child as CFTypeRef) };
            }
            unsafe { CFRelease(col as CFTypeRef) };
        }

        // Also try AXChildren path
        println!("\nAXChildren of browser:");
        let br_children = copy_el_array(br, "AXChildren");
        for (ci, &child) in br_children.iter().enumerate() {
            let r = fetch_str(child, "AXRole");
            let d = fetch_str(child, "AXDescription");
            println!("  Child[{}] role={} desc={}", ci, r, d);
            // One more level
            let gchildren = copy_el_array(child, "AXChildren");
            for (gi, &gc) in gchildren.iter().enumerate().take(4) {
                let r2 = fetch_str(gc, "AXRole");
                let d2 = fetch_str(gc, "AXDescription");
                println!("    GC[{}] role={} desc={}", gi, r2, d2);
                // Column rows inside scroll area
                let sc_children = copy_el_array(gc, "AXChildren");
                for (sci, &sc) in sc_children.iter().enumerate().take(4) {
                    let r3 = fetch_str(sc, "AXRole");
                    let d3 = fetch_str(sc, "AXDescription");
                    println!("      SC[{}] role={} desc={}", sci, r3, d3);
                    let sc2 = copy_el_array(sc, "AXRows");
                    for (s2i, &s2) in sc2.iter().enumerate().take(6) {
                        dump_element(s2, s2i, 10);
                    }
                    for &s2 in &sc2 {
                        unsafe { CFRelease(s2 as CFTypeRef) };
                    }
                    unsafe { CFRelease(sc as CFTypeRef) };
                }
                for &sc in &sc_children {
                    unsafe { CFRelease(sc as CFTypeRef) };
                }
                unsafe { CFRelease(gc as CFTypeRef) };
            }
            for &gc in &gchildren {
                unsafe { CFRelease(gc as CFTypeRef) };
            }
            unsafe { CFRelease(child as CFTypeRef) };
        }

        unsafe { CFRelease(br as CFTypeRef) };
    } else {
        println!("Could not find AXBrowser");
    }

    // ── AXCopyMultipleAttributeValues on an AXRow ───────────────────────
    println!("\n────────────────────────────────────────");
    println!("PART 3: AXUIElementCopyMultipleAttributeValues on AXRow");
    println!("────────────────────────────────────────");

    let sidebar2 = find_by_desc(win, "sidebar", 0);
    if let Some(outline) = sidebar2 {
        let rows = copy_el_array(outline, "AXRows");
        if let Some(&row) = rows.first() {
            println!("Testing on first sidebar AXRow:");
            test_multi_attr_extended(row);
        }
        for &row in &rows {
            unsafe { CFRelease(row as CFTypeRef) };
        }
        unsafe { CFRelease(outline as CFTypeRef) };
    }

    // Cleanup
    for &win2 in &windows {
        unsafe { CFRelease(win2 as CFTypeRef) };
    }
    unsafe { CFRelease(app_el as CFTypeRef) };
}

// ── Deep element dump ────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn dump_element(el: accessibility_sys::AXUIElementRef, idx: usize, indent: usize) {
    use core_foundation::base::{CFRelease, CFTypeRef};
    let pad = " ".repeat(indent);
    let role = fetch_str(el, "AXRole");
    let sub = fetch_str(el, "AXSubrole");
    let title = fetch_str(el, "AXTitle");
    let desc = fetch_str(el, "AXDescription");
    let val = fetch_str(el, "AXValue");
    let help = fetch_str(el, "AXHelp");
    println!(
        "{}[{}] role={} subrole={} title={} desc={} value={} help={}",
        pad, idx, role, sub, title, desc, val, help
    );
    dump_all_attrs(el, indent + 2);

    let children = copy_el_array(el, "AXChildren");
    for (ci, &child) in children.iter().enumerate() {
        let cr = fetch_str(child, "AXRole");
        let ct = fetch_str(child, "AXTitle");
        let cd = fetch_str(child, "AXDescription");
        let cv = fetch_str(child, "AXValue");
        println!(
            "{}  child[{}] role={} title={} desc={} value={}",
            pad, ci, cr, ct, cd, cv
        );
        dump_all_attrs(child, indent + 4);

        let gchildren = copy_el_array(child, "AXChildren");
        for (gi, &gc) in gchildren.iter().enumerate() {
            let gr = fetch_str(gc, "AXRole");
            let gt = fetch_str(gc, "AXTitle");
            let gd = fetch_str(gc, "AXDescription");
            let gv = fetch_str(gc, "AXValue");
            println!(
                "{}    gc[{}] role={} title={} desc={} value={}",
                pad, gi, gr, gt, gd, gv
            );
            dump_all_attrs(gc, indent + 6);
            unsafe { CFRelease(gc as CFTypeRef) };
        }
        unsafe { CFRelease(child as CFTypeRef) };
    }
}

#[cfg(target_os = "macos")]
fn test_multi_attr_extended(el: accessibility_sys::AXUIElementRef) {
    use accessibility_sys::{
        AXUIElementCopyAttributeNames, AXUIElementCopyMultipleAttributeValues,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
    };

    // First, get ALL attribute names for this element
    let mut names_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null_mut();
    let err = unsafe { AXUIElementCopyAttributeNames(el, &mut names_ref) };
    if err != 0 || names_ref.is_null() {
        return;
    }
    let name_arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(names_ref as _) };
    let all_names: Vec<String> = name_arr
        .into_iter()
        .filter_map(|item| item.downcast::<CFString>().map(|s| s.to_string()))
        .collect();

    println!("Available attributes on AXRow: {:?}", all_names);

    // Batch fetch all of them
    let cf_names: Vec<CFString> = all_names.iter().map(|a| CFString::new(a)).collect();
    let cf_refs: Vec<_> = cf_names.iter().map(|s| s.as_concrete_TypeRef()).collect();
    let names_arr2 = CFArray::from_copyable(&cf_refs);

    let mut result: CFTypeRef = std::ptr::null_mut();
    let err2 = unsafe {
        AXUIElementCopyMultipleAttributeValues(
            el,
            names_arr2.as_concrete_TypeRef(),
            0,
            &mut result as *mut _ as *mut _,
        )
    };
    println!("AXUIElementCopyMultipleAttributeValues err={}", err2);
    if err2 == 0 && !result.is_null() {
        let res_arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(result as _) };
        for (i, item) in res_arr.into_iter().enumerate() {
            let name = all_names.get(i).map(|s| s.as_str()).unwrap_or("?");
            let repr = if let Some(s) = item.downcast::<CFString>() {
                format!("String(\"{}\")", s.to_string())
            } else if let Some(b) = item.downcast::<CFBoolean>() {
                format!("Bool({})", bool::from(b))
            } else if let Some(n) = item.downcast::<CFNumber>() {
                format!("Number({})", n.to_i64().unwrap_or(-1))
            } else {
                format!("Other(type_id={})", item.type_of())
            };
            println!("  [{}] {} = {}", i, name, repr);
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn find_pid(name: &str) -> Option<i32> {
    let out = std::process::Command::new("pgrep")
        .arg("-x")
        .arg(name)
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
fn count_attr(el: accessibility_sys::AXUIElementRef, attr: &str) -> usize {
    copy_el_array(el, attr).len()
}

#[cfg(target_os = "macos")]
fn fetch_str(el: accessibility_sys::AXUIElementRef, attr: &str) -> String {
    use accessibility_sys::AXUIElementCopyAttributeValue;
    use core_foundation::{
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
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
        return format!("num:{}", n.to_i64().unwrap_or(-1));
    }
    format!("cftype:{}", cf.type_of())
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
        return;
    }
    let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(names_ref as _) };
    let names: Vec<String> = arr
        .into_iter()
        .filter_map(|item| item.downcast::<CFString>().map(|s| s.to_string()))
        .collect();
    for name in &names {
        let val = fetch_str(el, name);
        println!("{}{}: {}", pad, name, val);
    }
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

/// Find first element with given AXDescription recursively
#[cfg(target_os = "macos")]
fn find_by_desc(
    el: accessibility_sys::AXUIElementRef,
    desc: &str,
    depth: u32,
) -> Option<accessibility_sys::AXUIElementRef> {
    use core_foundation::base::{CFRetain, CFTypeRef};
    if depth > 8 {
        return None;
    }
    let d = fetch_str(el, "AXDescription");
    if d == format!("\"{}\"", desc) {
        unsafe { CFRetain(el as CFTypeRef) };
        return Some(el);
    }
    for child in copy_el_array(el, "AXChildren") {
        if let Some(found) = find_by_desc(child, desc, depth + 1) {
            unsafe { core_foundation::base::CFRelease(child as CFTypeRef) };
            return Some(found);
        }
        unsafe { core_foundation::base::CFRelease(child as CFTypeRef) };
    }
    None
}

/// Find first element with given AXRole recursively
#[cfg(target_os = "macos")]
fn find_by_role(
    el: accessibility_sys::AXUIElementRef,
    role: &str,
    depth: u32,
) -> Option<accessibility_sys::AXUIElementRef> {
    use core_foundation::base::{CFRelease, CFRetain, CFTypeRef};
    if depth > 8 {
        return None;
    }
    let r = fetch_str(el, "AXRole");
    if r == format!("\"{}\"", role) {
        unsafe { CFRetain(el as CFTypeRef) };
        return Some(el);
    }
    for child in copy_el_array(el, "AXChildren") {
        if let Some(found) = find_by_role(child, role, depth + 1) {
            unsafe { CFRelease(child as CFTypeRef) };
            return Some(found);
        }
        unsafe { CFRelease(child as CFTypeRef) };
    }
    None
}
