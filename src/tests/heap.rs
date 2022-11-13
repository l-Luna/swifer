use std::mem;
use std::sync::atomic::{AtomicU8, Ordering};
use crate::heap::{DynSized, GcCandidate, Heap};

use dyn_struct2::dyn_arg;
use dyn_struct_derive2::DynStruct;

// setup the heap allocated struct

#[repr(C)]
#[derive(Debug, DynStruct)]
struct MyUnsized{
    bad: [u8]
}

// check dropping
static DROP_COUNTER: AtomicU8 = AtomicU8::new(0);

impl Drop for MyUnsized{
    fn drop(&mut self){
        DROP_COUNTER.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl DynSized for MyUnsized{
    fn dyn_align() -> usize{
        mem::align_of::<u8>()
    }
}

impl GcCandidate for MyUnsized{
    fn collect_managed_pointers(&self) -> Vec<*const Self>{
        Vec::new()
    }

    fn adjust_ptrs(&mut self, _: impl Fn(&*const Self) -> *const Self){}
}

#[test]
fn test_basic_push_drop(){
    let mut heap = Heap::<MyUnsized>::new(100);
    heap.push(MyUnsized::new(dyn_arg!([1, 2, 3]))).unwrap();

    drop(heap);

    assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 1);

    let mut heap2 = Heap::<MyUnsized>::new(100);
    heap2.push(MyUnsized::new(dyn_arg!([4]))).unwrap();
    heap2.push(MyUnsized::new(dyn_arg!([5, 6, 7]))).unwrap();

    drop(heap2);

    assert_eq!(DROP_COUNTER.load(Ordering::Relaxed), 3);
}