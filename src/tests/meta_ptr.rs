
// Test a data type that stores type information in the heap's pointers, not inline

use std::ptr::null;
use crate::gc::{GcCandidate, ManagedMem};
use crate::gc::mas::MarkAndSweepMem;
use crate::heap::HeapPtr;

union PolyData{
    i_val: i64,
    ptr_val: *const PolyData, // no metadata in data
    nothing_val: ()
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct PolyPtr{
    ptr: *const PolyData,
    tag: PolyTag
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum PolyTag{
    Invalid, Int, Ptr, Nothing, Untyped
}

// GC behaviour

impl GcCandidate<PolyPtr> for PolyData{
    fn collect_managed_pointers(&self, this: &PolyPtr) -> Vec<PolyPtr> {
        if this.tag == PolyTag::Untyped{
            panic!("Untyped poly pointer provided as `this`!");
        }
        if this.tag == PolyTag::Invalid{
            panic!("Invalid poly pointer provided as `this`!");
        }
        if this.tag == PolyTag::Ptr{
            return vec![PolyPtr{
                ptr: unsafe{ self.ptr_val },
                tag: PolyTag::Untyped
            }];
        }
        return vec![];
    }

    fn adjust_ptrs(&mut self, adjust: impl Fn(&PolyPtr) -> PolyPtr, this: &PolyPtr) {
        if this.tag == PolyTag::Untyped{
            panic!("Untyped poly pointer provided as `this`!");
        }
        if this.tag == PolyTag::Invalid{
            panic!("Invalid poly pointer provided as `this`!");
        }
        if this.tag == PolyTag::Ptr{
            unsafe{ self.ptr_val = adjust(&PolyPtr { ptr: self.ptr_val, tag: PolyTag::Untyped }).ptr; }
        }
    }
}

impl HeapPtr<PolyData> for PolyPtr{
    fn from_raw_ptr(raw: *const PolyData) -> Self{
        return PolyPtr{
            ptr: raw,
            tag: PolyTag::Invalid
        }
    }

    fn to_raw_ptr(&self) -> *const PolyData{
        return self.ptr;
    }

    fn copy_meta(&mut self, other: &Self){
        self.tag = other.tag;
    }

    fn has_significant_meta() -> bool{
        return true;
    }

    fn eq_ignoring_meta(&self, other: &Self) -> bool {
        return self.ptr == other.ptr;
    }
}

#[test]
fn test_ptr_with_meta(){
    let mut heap = MarkAndSweepMem::<PolyData, PolyPtr>::new(500);

    let mut int = heap.push_with(Box::new(PolyData{ i_val: 1 }), |mut p| { p.tag = PolyTag::Int; p }).unwrap();
    let mut l = heap.push_with(Box::new(PolyData{ ptr_val: null() }), |mut p| { p.tag = PolyTag::Ptr; p }).unwrap();
    let mut r = heap.push_with(Box::new(PolyData{ ptr_val: null() }), |mut p| { p.tag = PolyTag::Ptr; p }).unwrap();
    let mut tn = heap.push_with(Box::new(PolyData{ ptr_val: null() }), |mut p| { p.tag = PolyTag::Ptr; p }).unwrap();
    let mut n = heap.push_with(Box::new(PolyData{ nothing_val: () }), |mut p| { p.tag = PolyTag::Nothing; p }).unwrap();

    { let x = heap.get_by(&l).unwrap(); x.ptr_val = r.ptr; }
    { let y = heap.get_by(&r).unwrap(); y.ptr_val = l.ptr; }
    { let z = heap.get_by(&tn).unwrap(); z.ptr_val = n.ptr; }

    heap.gc(vec![&mut int, &mut l, &mut r, &mut tn, &mut n], vec![]);
    assert_eq!(heap.len(), 5);

    heap.gc(vec![&mut l, &mut tn], vec![&mut r, &mut n]);
    assert_eq!(heap.len(), 4);

    heap.gc(vec![&mut tn], vec![&mut n]);
    assert_eq!(heap.len(), 2);

    heap.gc(vec![&mut n], vec![]);
    assert_eq!(heap.len(), 1);
}