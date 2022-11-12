use std::{alloc, mem, ptr};
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
}

unsafe impl<T: Sized> DynSized for [T]{
    fn dyn_align() -> usize {
        return mem::align_of::<T>();
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
        let size = mem::size_of_val(v.as_ref());
        // check we can allocate
        if self.cap - self.used < size{
            return false;
        }
        unsafe{
            // get the raw source pointer (with size metadata)
            let raw = Box::into_raw(v);
            // find the destination location
            let dest_ptr: *mut () = self.head.as_ptr().offset(self.used as isize);
            // add the metadata of the source pointer (e.g. object size) to get the fat target pointer
            let dest_ptr: *mut T = dest_ptr.with_metadata_of(raw);
            // copy the bytes of the source to the target
            // *const u8 is required as we specify size in bytes
            (dest_ptr as *mut u8).copy_from(raw as *const u8, size);
            // deallocate the box's memory
            alloc::dealloc(raw as *mut u8, alloc::Layout::for_value_raw(raw));
            // keep track of the new entry
            self.indexes.push(Ptr::from_raw_ptr(dest_ptr));
        }
        self.used += size;
        return true;
    }

    pub fn get(&self, idx: usize) -> &T{
        unsafe{
            return self.indexes[idx].to_raw_ptr().as_ref().expect("Heap::get: GcPtr returned null");
        }
    }

    pub fn len(&self) -> usize{
        return self.indexes.len();
    }

    pub fn for_each(&self, mut cb: impl FnMut(&T)){
        for i in 0..self.len(){
            cb(self.get(i));
        }
    }
}

impl<T: ?Sized + GcCandidate, Ptr: GcPtr<T>> Drop for Heap<T, Ptr>{
    fn drop(&mut self){
        // drop each object
        for i in 0..self.len(){
            let ptr = &self.indexes[i];
            let raw = ptr.to_raw_ptr() as *mut T;
            unsafe{
                raw.drop_in_place();
            }
        }
        unsafe{
            // then deallocate the whole thing
            alloc::dealloc(self.head.as_ptr() as *mut u8, alloc::Layout::array::<()>(self.cap).unwrap());
        }
    }
}