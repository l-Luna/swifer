#![feature(layout_for_ptr)]

pub mod heap;
pub mod gc;

#[cfg(test)]
mod tests{
    use crate::heap::Heap;

    #[derive(Debug)]
    struct MyUnsized{
        bad: [u8]
    }

    impl Drop for MyUnsized{
        fn drop(&mut self){
            println!("Dropping!");
        }
    }

    #[test]
    fn it_works(){
        //let heap = Heap::new(100);
        //let ux = Box::new(MyUnsized{ bad: [1] as [u8] });
    }
}
