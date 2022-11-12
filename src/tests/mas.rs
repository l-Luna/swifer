use std::mem;
use std::sync::Mutex;
use dyn_struct2::dyn_arg;
use dyn_struct_derive2::DynStruct;
use crate::gc::ManagedMem;
use crate::gc::mas::{MarkAndSweepMem, MasCandidate};
use crate::heap::{DynSized, GcCandidate, GcPtr};
use crate::tests::mas::MyDataValue::{Pointer, Int, Nothing};

// setup the data types

#[derive(Debug)]
enum MyDataValue{
    Int(i32),
    Pointer(MyPointer),
    Nothing
}

#[repr(C)]
#[derive(Debug, DynStruct)]
struct MyUnsized{
    marked: bool,
    values: [MyDataValue]
}

#[derive(Clone, Eq, PartialEq, Debug)]
struct MyPointer(*const MyUnsized);

unsafe impl DynSized for MyUnsized{
    fn dyn_align() -> usize{
        return mem::align_of::<MyDataValue>();
    }
}

impl GcCandidate<MyPointer> for MyUnsized{
    fn collect_managed_pointers(&self) -> Vec<MyPointer>{
        return self.values.iter().filter_map(|x| match x{
            Pointer(p) => Some(p.clone()),
            _ => None
        }).collect();
    }

    fn adjust_ptrs(&mut self, adjust: impl Fn(MyPointer) -> MyPointer){
        for i in 0..self.values.len(){
            if let Pointer(p) = &self.values[i]{
                self.values[i] = Pointer(adjust(p.clone()));
            }
        }
    }
}

impl GcPtr<MyUnsized> for MyPointer{
    fn from_raw_ptr(raw: *const MyUnsized) -> Self{
        return MyPointer(raw);
    }

    fn to_raw_ptr(&self) -> *const MyUnsized{
        return self.0;
    }
}

impl MasCandidate<MyPointer> for MyUnsized{
    fn is_marked(&self) -> bool {
        return self.marked;
    }

    fn set_marked(&mut self, marked: bool) {
        self.marked = marked;
    }
}

// use dropping to check what has been deallocated at what point
static DROPPED: Mutex<Vec<i32>> = Mutex::new(Vec::new());

impl Drop for MyUnsized{
    fn drop(&mut self){
        if let Int(x) = self.values[0]{
            DROPPED.lock().unwrap().push(x);
        }
    }
}

#[test]
fn test_mark_and_sweep(){
    // set up a heap with cycles
    let mut heap = MarkAndSweepMem::<MyUnsized, MyPointer>::new(100);

    let root = heap.push(MyUnsized::new(false, dyn_arg!([MyDataValue::Int(1), MyDataValue::Nothing]))).unwrap();
    let l = heap.push(MyUnsized::new(false, dyn_arg!([Int(0), Nothing]))).unwrap();
    let r = heap.push(MyUnsized::new(false, dyn_arg!([Int(3), Nothing]))).unwrap();
    let s = heap.push(MyUnsized::new(false, dyn_arg!([Int(8), Nothing]))).unwrap();
    let n = heap.push(MyUnsized::new(false, dyn_arg!([Int(14)]))).unwrap();

    // root -> l
    { heap.get_by(&root).unwrap().values[1] = Pointer(l.clone()); }
    // l -> r, r -> l
    { heap.get_by(&l).unwrap().values[1] = Pointer(r.clone()); }
    { heap.get_by(&r).unwrap().values[1] = Pointer(l.clone()); }
    // s -> s
    { heap.get_by(&s).unwrap().values[1] = Pointer(s.clone()); }
    // n -> nothing

    heap.gc(vec![root, n.clone()]);
    { assert!(DROPPED.lock().unwrap().eq(&vec![8])); }

    heap.gc(vec![l, n.clone()]);
    { assert!(DROPPED.lock().unwrap().eq(&vec![8, 1])); }

    heap.gc(vec![n.clone()]);
    { assert!(DROPPED.lock().unwrap().eq(&vec![8, 1, 0, 3])); }

    heap.gc(vec![]);
    { assert!(DROPPED.lock().unwrap().eq(&vec![8, 1, 0, 3, 14])); }
}