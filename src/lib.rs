#![feature(layout_for_ptr)]
#![feature(set_ptr_value)]

pub mod heap;
pub mod gc;

#[cfg(test)]
mod tests{
    use std::mem::align_of;
    use crate::heap::{DynSized, GcCandidate, Heap};

    use dyn_struct2::dyn_arg;
    use dyn_struct_derive2::DynStruct;

    #[repr(C)]
    #[derive(Debug, DynStruct)]
    struct MyUnsized{
        bad: [u8]
    }

    impl Drop for MyUnsized{
        fn drop(&mut self){
            println!("Dropping!");
        }
    }

    unsafe impl DynSized for MyUnsized{
        fn dyn_align() -> usize{
            align_of::<u8>()
        }
    }

    impl GcCandidate for MyUnsized{
        fn collect_managed_pointers(&self) -> Vec<*const Self>{
            Vec::new()
        }
    }

    #[test]
    fn it_works(){
        let mut heap = Heap::<MyUnsized>::new(100);
        let ux = MyUnsized::new(dyn_arg!([1, 2, 3]));
        heap.push(ux);
        // Dropping!
    }
}
