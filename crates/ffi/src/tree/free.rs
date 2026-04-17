use crate::convert::string::free_c_string;
use crate::types::{AdNode, AdNodeTree};
use std::os::raw::c_char;
use std::ptr;

unsafe fn free_c_string_array(arr: *mut *mut c_char, count: u32) {
    if arr.is_null() {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(arr, count as usize);
    for p in slice.iter_mut() {
        free_c_string(*p);
    }
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        arr,
        count as usize,
    )));
}

unsafe fn free_node_fields(node: &mut AdNode) {
    free_c_string(node.ref_id as *mut c_char);
    free_c_string(node.role as *mut c_char);
    free_c_string(node.name as *mut c_char);
    free_c_string(node.value as *mut c_char);
    free_c_string(node.description as *mut c_char);
    free_c_string(node.hint as *mut c_char);
    free_c_string_array(node.states, node.state_count);
    node.ref_id = ptr::null();
    node.role = ptr::null();
    node.name = ptr::null();
    node.value = ptr::null();
    node.description = ptr::null();
    node.hint = ptr::null();
    node.states = ptr::null_mut();
    node.state_count = 0;
}

/// # Safety
/// `tree` must be null or point to a valid `AdNodeTree` previously returned
/// by `flatten_tree` or `ad_get_tree`. After this call the tree is zeroed.
#[no_mangle]
pub unsafe extern "C" fn ad_free_tree(tree: *mut AdNodeTree) {
    crate::ffi_try::trap_panic_void(|| unsafe {
        if tree.is_null() {
            return;
        }
        let tree = &mut *tree;
        if tree.nodes.is_null() {
            return;
        }
        let nodes = std::slice::from_raw_parts_mut(tree.nodes, tree.count as usize);
        for node in nodes.iter_mut() {
            free_node_fields(node);
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            tree.nodes,
            tree.count as usize,
        )));
        tree.nodes = ptr::null_mut();
        tree.count = 0;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_null_tree_is_noop() {
        unsafe { ad_free_tree(std::ptr::null_mut()) };
    }
}
