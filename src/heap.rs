//! The heap data structure, alongside basic traits used by garbage collectors.

use std::{alloc, mem};
use std::marker::PhantomData;
use std::ptr::NonNull;

/// A fixed-capacity contiguous vector of possibly-unsized data.
pub struct Heap<T, Ptr = *const T>
    where T: ?Sized + DynSized, Ptr: HeapPtr<T>
{
    head: NonNull<u8>, // T is ?Sized, so NonNull<T> would need metadata that doesn't exist yet
    cap: usize,
    used: usize,
    indexes: Vec<Ptr>,
    _phantom: PhantomData<T>
}

/// A (possibly-unsized) value that provides certain information about its memory layout.
///
/// Automatically implemented for sized types and slices.
pub unsafe trait DynSized{
    /// Returns the alignment of values of this type.
    fn dyn_align() -> usize;
}

/// A pointer to a value in managed memory, usable by heaps.
///
/// By default, raw `*const` pointers are used. You may want to implement this yourself if:
///  - It's more convenient to do so, e.g. you already have a smart pointer type.
///  - You want to store additional metadata, e.g. types, that are relevant for garbage collection.
///
/// In the latter case, additionally implement [GcPtr::copy_meta], [GcPtr::has_significant_meta],
/// and [GcPtr::eq_ignoring_meta].
pub trait HeapPtr<T: ?Sized>: Eq + Clone{
    /// Create an instance of this pointer type with the target and size information given.
    fn from_raw_ptr(raw: *const T) -> Self;
    /// Gets a raw pointer with the same target and size information as this pointer.
    fn to_raw_ptr(&self) -> *const T;

    /// Copies any additional metadata from the given other pointer, such as types.
    fn copy_meta(&mut self, _other: &Self){
        // no-op
    }
    /// Whether this pointer type stores any additional metadata that must be copied.
    /// Garbage collectors may opt not to track metadata (i.e. ignore [GcPtr::copy_meta]) if
    /// this is false.
    fn has_significant_meta() -> bool{
        return false;
    }
    /// Returns whether this pointer is equal to the other pointer, ignoring any additional
    /// metadata, i.e. whether they point to the same memory.
    fn eq_ignoring_meta(&self, other: &Self) -> bool{
        return self == other;
    }
}

//////////////// impls

impl<T: ?Sized> HeapPtr<T> for *const T{
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

impl<T: ?Sized + DynSized, Ptr: HeapPtr<T>> Heap<T, Ptr>{

    /// Creates a new heap with the given capacity in bytes.
    pub fn new(size: usize) -> Heap<T, Ptr>{
        let layout = alloc::Layout::from_size_align(size, T::dyn_align()).expect("Invalid layout for new Heap");
        let head = unsafe{ alloc::alloc(layout) };
        let nn_head = match NonNull::new(head){
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

    /// Pushes an object onto the end of this heap, returning a pointer to it,
    /// or `None` if this heap is full.
    ///
    /// The given `with` function is applied to the pointer before saving, for e.g.
    /// adding extra metadata.
    pub fn push_with(&mut self, v: Box<T>, with: impl FnOnce(Ptr) -> Ptr) -> Option<Ptr>{
        let size = mem::size_of_val(v.as_ref());
        // check we can allocate
        if self.cap - self.used < size{
            return None;
        }
        let new_ptr: Ptr;
        unsafe{
            // get the raw source pointer (with size metadata)
            let raw = Box::into_raw(v);
            // find the destination location
            let dest_ptr: *mut u8 = self.head.as_ptr().offset(self.used as isize);
            // add the metadata of the source pointer (e.g. object size) to get the fat target pointer
            let dest_ptr: *mut T = dest_ptr.with_metadata_of(raw);
            // copy the bytes of the source to the target
            // *const u8 is required as we specify size in bytes
            (dest_ptr as *mut u8).copy_from(raw as *const u8, size);
            // deallocate the box's memory
            alloc::dealloc(raw as *mut u8, alloc::Layout::for_value_raw(raw));
            // keep track of the new entry
            new_ptr = with(Ptr::from_raw_ptr(dest_ptr));
            self.indexes.push(new_ptr.clone());
        }
        self.used += size;
        return Some(new_ptr);
    }

    /// Pushes an object onto the end of this heap, returning a pointer to it,
    /// or `None` if this heap is full.
    pub fn push(&mut self, v: Box<T>) -> Option<Ptr>{
        return self.push_with(v, |x| x);
    }

    /// Returns a reference to the value at the given index.
    pub fn get(&self, idx: usize) -> &T{
        unsafe{
            return self.indexes[idx].to_raw_ptr().as_ref().expect("Heap::get: GcPtr returned null");
        }
    }

    /// Returns a mutable reference to the value at the given index.
    pub fn get_mut(&mut self, idx: usize) -> &mut T{
        unsafe{
            return (self.indexes[idx].to_raw_ptr() as *mut T).as_mut().expect("Heap::get_mut: GcPtr returned null");
        }
    }

    /// Returns a mutable reference to the value at the given pointer, or `None`
    /// if that pointer does not point to a value in this heap.
    pub fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T>{
        return self.indexes.iter().position(|p| p == ptr).map(|x| self.get_mut(x));
    }

    /// Moves the element at the given index out of this heap, returning it (contained in a box)
    /// and its former pointer.
    ///
    /// Note that this does not allow new values to be allocated in their place; use
    /// [Heap::reset] if that is necessary.
    pub fn take(&mut self, idx: usize) -> (Box<T>, Ptr){
        // need to preserve order because this might be called in a (reversed) loop
        let ptr = self.indexes.remove(idx);
        unsafe{
            // get the raw source pointer with size metadata
            let src: *const T = ptr.to_raw_ptr();
            // find the size
            let size = mem::size_of_val_raw(src);
            // allocate the target memory
            let dest: *mut u8 = alloc::alloc(alloc::Layout::for_value_raw(src));
            // add size info to the destination pointer
            let dest: *mut T = dest.with_metadata_of(src);
            // copy the object's data into the destination
            (dest as *mut u8).copy_from(src as *const u8, size);
            // convert to a box and return
            return (Box::from_raw(dest), ptr);
        }
    }

    /// Returns the number of values stored in this heap.
    pub fn len(&self) -> usize{
        return self.indexes.len();
    }

    /// Returns whether the given pointer points to a value in this heap.
    pub fn contains_ptr(&self, ptr: &Ptr) -> bool{
        return self.indexes.contains(ptr);
    }

    /// Returns a pointer equivalent to the one given, but with any additional metadata
    /// know by this heap, using [GcPtr::eq_ignoring_meta].
    pub fn to_full_ptr(&self, ptr: &Ptr) -> Ptr{
        return self.indexes.iter().filter(|x| x.eq_ignoring_meta(&ptr)).next().clone().unwrap().clone();
    }

    /// Runs the given function over every value in this heap.
    pub fn for_each(&self, mut cb: impl FnMut(&T, &Ptr)){
        for i in 0..self.len(){
            cb(self.get(i), &self.indexes[i]);
        }
    }

    /// Runs the given function over every value in this heap, allowing mutation.
    pub fn for_each_mut(&mut self, mut cb: impl FnMut(&mut T, &Ptr)){
        for i in 0..self.len(){
            let ptr = &self.indexes[i].clone();
            cb(self.get_mut(i), ptr);
        }
    }

    /// Empties this heap, dropping all values and allowing new ones to be pushed in their place.
    pub fn reset(&mut self){
        for i in 0..self.len(){
            let ptr = &self.indexes[i];
            let raw = ptr.to_raw_ptr() as *mut T;
            unsafe{
                raw.drop_in_place();
            }
        }
        self.used = 0;
    }

    /// Returns the capacity of this heap, in bytes.
    pub fn capacity(&self) -> usize{
        return self.cap;
    }
}

impl<T: ?Sized + DynSized, Ptr: HeapPtr<T>> Drop for Heap<T, Ptr>{
    fn drop(&mut self){
        // drop each object
        self.reset();
        unsafe{
            // then deallocate the whole thing
            alloc::dealloc(self.head.as_ptr(), alloc::Layout::array::<()>(self.cap).unwrap());
        }
    }
}