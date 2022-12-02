//! Garbage collectors and GC-managed memory.

use crate::heap::{DynSized, Heap, HeapPtr};

pub mod mas;

/// A memory space managed by a garbage collector.
///
/// Values can be pushed, and later accessed by index or pointer; all accesses are
/// validated. Additional metadata may be stored for values if necessary for
/// the values or GC. Space can be freed using [ManagedMem::gc].
///
/// Values may or may not be sized; they must opt-in to garbage collection.
///
/// By default, raw constant pointers (`*const T`) are used. Another type may
/// be used, so long as it implements [GcPtr].
pub trait ManagedMem<T, Ptr = *const T>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: HeapPtr<T>
{
    /// Pushes an object onto the end, returning a pointer to it, or `None` if this is full.
    fn push(&mut self, v: Box<T>) -> Option<Ptr>;

    /// Pushes an object onto the end, returning a pointer to it, or `None` if this is full.
    ///
    /// The given `with` function is applied to the pointer before saving, for e.g.
    /// adding extra metadata.
    fn push_with(&mut self, v: Box<T>, with: impl FnOnce(Ptr) -> Ptr) -> Option<Ptr>;

    /// Returns a reference to the value at the given index.
    fn get(&self, idx: usize) -> &T;

    /// Returns a mutable reference to the value at the given index.
    fn get_mut(&mut self, idx: usize) -> &mut T;

    /// Returns a mutable reference to the value at the given pointer, or `None`
    /// if that pointer does not point to a value in this memory.
    fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T>;

    /// Returns the number of values stored.
    fn len(&self) -> usize;

    /// Returns whether the given pointer points to a value in this memory.
    fn contains_ptr(&self, ptr: &Ptr) -> bool;

    /// Runs the given function over every value.
    fn for_each(&self, cb: impl FnMut(&T, &Ptr));

    // TODO: is this the right representation of roots?
    /// Trigger garbage collection, removing any values unreachable from the given `roots`.
    ///
    /// Values in both `roots` and `weaks` are updated if the value they point to are moved,
    /// but only values in `roots` can cause another value to become reachable.
    fn gc(&mut self, roots: Vec<&mut Ptr>, weaks: Vec<&mut Ptr>);
}

/// A value in managed memory that may point to other managed values, keeping them reachable.
pub trait GcCandidate<Ptr = *const Self>: DynSized
    where Ptr: HeapPtr<Self>
{
    /// Collects all pointers in this value to other garbage-collected objects.
    /// Pointers to unmanaged memory must not be included.
    fn collect_managed_pointers(&self, this: &Ptr) -> Vec<Ptr>;
    /// Replaces all managed pointers within this value according to the given function
    /// (e.g. after this value's pointees have been moved).
    fn adjust_ptrs(&mut self, adjust: impl Fn(&Ptr) -> Ptr, this: &Ptr);
}

// No-GC memory, delegates directly to the (single) heap.

/// A simple implementation of [ManagedMem] that does not implement garbage collection.
///
/// [ManagedMem::gc] calls have no effect, and memory is not freed until this is dropped.
pub struct NoGcMem<T, Ptr = *const T>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: HeapPtr<T>
{
    heap: Heap<T, Ptr>
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: HeapPtr<T>> NoGcMem<T, Ptr>{
    /// Creates a new `NoGcMem` with the given capacity in bytes.
    pub fn new(size: usize) -> Self{
        return NoGcMem{
            heap: Heap::new(size)
        };
    }
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: HeapPtr<T>> ManagedMem<T, Ptr> for NoGcMem<T, Ptr>{
    fn push(&mut self, v: Box<T>) -> Option<Ptr>{
        return self.heap.push(v);
    }

    fn push_with(&mut self, v: Box<T>, with: impl FnOnce(Ptr) -> Ptr) -> Option<Ptr> {
        return self.heap.push_with(v, with);
    }

    fn get(&self, idx: usize) -> &T{
        return self.heap.get(idx);
    }

    fn get_mut(&mut self, idx: usize) -> &mut T {
        return self.heap.get_mut(idx);
    }

    fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T> {
        return self.heap.get_by(ptr);
    }

    fn len(&self) -> usize{
        return self.heap.len();
    }

    fn contains_ptr(&self, ptr: &Ptr) -> bool{
        return self.heap.contains_ptr(ptr);
    }

    fn for_each(&self, cb: impl FnMut(&T, &Ptr)){
        self.heap.for_each(cb);
    }

    fn gc(&mut self, _roots: Vec<&mut Ptr>, _weaks: Vec<&mut Ptr>){
        // no-op
    }
}