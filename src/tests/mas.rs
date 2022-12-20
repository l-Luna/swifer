use std::mem;
use std::sync::Mutex;
use dyn_struct2::dyn_arg;
use dyn_struct_derive2::DynStruct;
use crate::gc::{GcCandidate, ManagedMem};
use crate::gc::mas::MarkAndSweepMem;
use crate::heap::{DynSized, HeapPtr};
use crate::tests::mas::MyDataValue::{Int, Nothing, Pointer};

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
    values: [MyDataValue]
}

impl MyUnsized{
    pub fn new_u<const N: usize>(values: [MyDataValue; N]) -> Box<MyUnsized>{
        return MyUnsized::new(dyn_arg!(values));
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct MyPointer(*const MyUnsized);

unsafe impl DynSized for MyUnsized{
    fn dyn_align() -> usize{
        return mem::align_of::<MyDataValue>();
    }
}

impl GcCandidate<MyPointer> for MyUnsized{
    fn collect_managed_pointers(&self, _this: &MyPointer) -> Vec<MyPointer>{
        return self.values.iter().filter_map(|x| match x{
            Pointer(p) => Some(p.clone()),
            _ => None
        }).collect();
    }

    fn adjust_ptrs(&mut self, adjust: impl Fn(&MyPointer) -> MyPointer, _this: &MyPointer){
        for i in 0..self.values.len(){
            if let Pointer(p) = &self.values[i]{
                self.values[i] = Pointer(adjust(p));
            }
        }
    }
}

impl HeapPtr<MyUnsized> for MyPointer{
    fn from_raw_ptr(raw: *const MyUnsized) -> Self{
        return MyPointer(raw);
    }

    fn to_raw_ptr(&self) -> *const MyUnsized{
        return self.0;
    }

    fn copy_meta(&mut self, _other: &MyPointer){}
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
    let mut heap = MarkAndSweepMem::<MyUnsized, MyPointer>::new(500);

    let mut root = heap.push(MyUnsized::new_u([Int(1), Nothing])).unwrap();
    let mut l = heap.push(MyUnsized::new_u([Int(0), Nothing])).unwrap();
    let mut r = heap.push(MyUnsized::new_u([Int(3), Nothing])).unwrap();
    let mut s = heap.push(MyUnsized::new_u([Int(8), Nothing])).unwrap();
    let mut n = heap.push(MyUnsized::new_u([Int(14)])).unwrap();

    // root -> l
    { heap.get_by(&root).unwrap().values[1] = Pointer(l.clone()); }
    // l -> r, r -> l
    { heap.get_by(&l).unwrap().values[1] = Pointer(r.clone()); }
    { heap.get_by(&r).unwrap().values[1] = Pointer(l.clone()); }
    // s -> s
    { heap.get_by(&s).unwrap().values[1] = Pointer(s.clone()); }
    // n -> nothing
    unsafe{
        heap.gc(vec![&mut root, &mut l, &mut r, &mut s, &mut n], vec![]);
        {
            assert!(DROPPED.lock().unwrap().eq(&vec![]));
            assert_eq!(heap.len(), 5); //root, l, r, s, n
        }

        heap.gc(vec![&mut root, &mut n], vec![&mut l, &mut r]);
        {
            assert!(DROPPED.lock().unwrap().eq(&vec![8]));
            assert_eq!(heap.len(), 4); //root, l, r, n
        }

        heap.gc(vec![&mut l, &mut n], vec![]);
        {
            assert!(DROPPED.lock().unwrap().eq(&vec![8, 1]));
            assert_eq!(heap.len(), 3); //l, r, n
        }

        heap.gc(vec![&mut n], vec![]);
        {
            assert!(DROPPED.lock().unwrap().eq(&vec![8, 1, 0, 3]));
            assert_eq!(heap.len(), 1); //n
        }

        heap.gc(vec![], vec![]);
        {
            assert!(DROPPED.lock().unwrap().eq(&vec![8, 1, 0, 3, 14]));
            assert_eq!(heap.len(), 0);
        }
    }
}