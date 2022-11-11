use std::{alloc, mem};
use std::marker::PhantomData;
use std::ptr::NonNull;

/// A fixed-capacity contiguous vector of possibly-unsized data.
pub struct Heap<T, Ptr = *const T>
    where T: ?Sized + GcCandidate, Ptr: GcPtr<T>
{
    head: NonNull<()>, // T is ?Sized, so NonNull<T> would need metadata that doesn't exist yet
    cap: usize,
    used: usize,
    indexes: Vec<Ptr>,
    _phantom: PhantomData<T>
}

/// A (possibly-unsized) value that provides certain information about its memory layout.
/// Automatically implemented for sized types.
pub unsafe trait DynSized{
    fn dyn_align() -> usize;
    fn dyn_size(&self) -> usize;
}

/// A pointer to a value in managed memory, usable by heaps.
/// Automatically implemented for `*const` pointers. Optionally implement on your own smart pointer type.
pub trait GcPtr<T: ?Sized>: Eq{
    fn from_raw_ptr(raw: *const T) -> Self;
    fn to_raw_ptr(&self) -> *const T;
}

/// A value in managed memory that may point to other managed values, keeping them reachable.
pub trait GcCandidate<Ptr = *const Self>: DynSized
    where Ptr: GcPtr<Self>
{
    fn collect_managed_pointers(&self) -> Vec<Ptr>;
}

//////////////// impls

impl<T: ?Sized> GcPtr<T> for *const T{
    fn from_raw_ptr(raw: *const T) -> Self { raw }
    fn to_raw_ptr(&self) -> *const T { *self }
}

unsafe impl<T: Sized> DynSized for T{
    fn dyn_align() -> usize{
        return mem::align_of::<T>();
    }
    fn dyn_size(&self) -> usize{
        return mem::size_of::<T>();
    }
}

unsafe impl<T: Sized> DynSized for [T]{
    fn dyn_align() -> usize {
        return mem::align_of::<T>();
    }
    fn dyn_size(&self) -> usize {
        return mem::size_of::<T>() * self.len();
    }
}

impl<T: ?Sized + GcCandidate, Ptr: GcPtr<T>> Heap<T, Ptr>{

    //copy, iter, clear
    pub fn new(size: usize) -> Heap<T, Ptr>{
        let layout = alloc::Layout::from_size_align(size, T::dyn_align()).expect("Invalid layout for new Heap");
        let head = unsafe{ alloc::alloc(layout) };
        let nn_head = match NonNull::new(head as *mut ()){
            None => alloc::handle_alloc_error(layout),
            Some(p) => p
        };
        return Heap{
            head: nn_head,
            cap: size,
            used: 0,
            indexes: vec![],
            _phantom: PhantomData
        };
    }

    // false = OOM
    pub fn push(&mut self, v: Box<T>) -> bool{
        let size = v.dyn_size();
        if self.cap - self.used < size{
            return false;
        }
        unsafe{
            let raw = Box::into_raw(v);
            let src_ptr = raw as *const ();
            let dest_ptr = self.head.as_ptr().offset(self.used as isize);
            dest_ptr.copy_from(src_ptr, size);
            alloc::dealloc(raw as *mut u8, alloc::Layout::for_value_raw(raw));
        }
        self.used += size;
        return true;
    }
}