use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::swap;
use crate::gc::ManagedMem;
use crate::heap::{GcCandidate, GcPtr, Heap};

// Mark and Sweep GC
// Traces all reachable objects, marking them; then copies all marked objects to a new heap, updating their pointers

pub struct MarkAndSweepMem<T, Ptr = *const T>
    where T: ?Sized + MasCandidate<Ptr>, Ptr: GcPtr<T>
{
    active: Heap<T, Ptr>,
    inactive: Heap<T, Ptr>
}

pub trait MasCandidate<Ptr: GcPtr<Self>>: GcCandidate<Ptr>{
    fn is_marked(&self) -> bool;
    fn set_marked(&mut self, marked: bool);
}

impl<T: ?Sized + MasCandidate<Ptr>, Ptr: GcPtr<T>> MarkAndSweepMem<T, Ptr>{
    pub fn new(size: usize) -> Self{
        return MarkAndSweepMem{
            active: Heap::new(size),
            inactive: Heap::new(size)
        };
    }
}

//////////////// impls

impl<T: ?Sized + MasCandidate<Ptr>, Ptr: GcPtr<T>> ManagedMem<T, Ptr> for MarkAndSweepMem<T, Ptr>{
    fn push(&mut self, v: Box<T>) -> Option<Ptr>{
        return self.active.push(v);
    }

    fn get(&self, idx: usize) -> &T{
        return self.active.get(idx);
    }

    fn get_mut(&mut self, idx: usize) -> &mut T{
        return self.active.get_mut(idx);
    }

    fn get_by(&mut self, ptr: &Ptr) -> Option<&mut T>{
        return self.active.get_by(ptr);
    }

    fn len(&self) -> usize{
        return self.active.len();
    }

    fn contains_ptr(&self, ptr: &Ptr) -> bool {
        return self.active.contains_ptr(ptr);
    }

    fn for_each(&self, cb: impl FnMut(&T)){
        self.active.for_each(cb);
    }

    fn gc(&mut self, roots: Vec<&mut Ptr>){
        // mark phase: mark every reachable object
        let mut count = 0;
        for root in &roots{
            count += mark_reachable(&mut self.active, root);
        }
        // sweep phase: copy marked objects to new heap and update pointers
        let mut rel: HashMap<HashWrap<T, Ptr>, HashWrap<T, Ptr>> = HashMap::with_capacity(count);
        for i in (0..self.active.len()).rev(){
            let (obj, old_ptr): (Box<T>, Ptr) = self.active.take(i);
            if obj.is_marked(){
                match self.inactive.push(obj){
                    Some(new_ptr) => rel.insert(HashWrap::new(old_ptr), HashWrap::new(new_ptr)),
                    None => panic!("Mark and Sweep: could not allocate space in inactive heap for object")
                };
            }else{
                drop(obj);
            }
        }
        let find = |p: &Ptr| rel.get(&HashWrap::new(p.clone())).expect("Could not find updated pointer!").ptr.clone();
        self.inactive.for_each_mut(|o: &mut T| o.adjust_ptrs(find));
        // unmark everything
        self.inactive.for_each_mut(|o: &mut T| o.set_marked(false));
        // reset the active heap - should not drop anything, since everything has been moved
        self.active.reset();
        // and swap them
        swap(&mut self.active, &mut self.inactive);
        // update root pointers
        for root in roots{
            *root = find(root);
        }
    }
}

fn mark_reachable<T: ?Sized + MasCandidate<Ptr>, Ptr: GcPtr<T>>(heap: &mut Heap<T, Ptr>, root: &Ptr) -> usize{
    let mut count = 0;
    // unprocessed objects
    let mut stack: Vec<Ptr> = Vec::with_capacity(5);
    stack.push(root.clone());
    while let Some(current) = stack.pop(){
        if let Some(obj) = heap.get_by(&current){
            // if not already marked,
            if !obj.is_marked(){
                // mark the object
                obj.set_marked(true);
                count += 1;
                // schedule every pointee for marking
                for ptr in obj.collect_managed_pointers(){
                    stack.push(ptr);
                }
            }
        }
    }
    return count;
}

// allow using HashMap over !Hash Ptr

struct HashWrap<T, Ptr>
    where T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>
{
    ptr: Ptr,
    _phantom: PhantomData<T>
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> HashWrap<T, Ptr>{
    fn new(ptr: Ptr) -> Self{
        return HashWrap{
            ptr,
            _phantom: PhantomData
        };
    }
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> Hash for HashWrap<T, Ptr>{
    fn hash<H: Hasher>(&self, state: &mut H){
        self.ptr.to_raw_ptr().hash(state)
    }
}

// must be written manually due to ?Sized bound (???)
impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> PartialEq for HashWrap<T, Ptr>{
    fn eq(&self, other: &Self) -> bool{
        return self.ptr == other.ptr;
    }
}

impl<T: ?Sized + GcCandidate<Ptr>, Ptr: GcPtr<T>> Eq for HashWrap<T, Ptr>{}