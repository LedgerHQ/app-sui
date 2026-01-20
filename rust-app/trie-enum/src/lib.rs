#![no_std]
#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(const_refs_to_cell)]

#[cfg(test)]
extern crate std;
#[cfg(test)]
use std::println;

pub use paste::paste;

#[derive(Clone, Copy, Debug)]
pub enum TrieInstruction<T: Copy> {
    Terminal(Option<T>),
    Node(u8, usize),
}

#[derive(Debug)]
pub struct Trie<T: Copy, const N: usize>([TrieInstruction<T>; N]);

#[derive(Debug, Clone, Copy)]
pub struct TrieCursor<'a, T: Copy, const N: usize>(&'a Trie<T, N>, usize);

impl<'a, T: Copy + core::fmt::Debug, const N: usize> TrieCursor<'a, T, N> {
    pub fn step(&self, input: u8) -> Option<TrieCursor<'a, T, N>> {
        let Trie(ref trie) = self.0;
        let mut n = self.1;
        if let TrieInstruction::Terminal(_) = &trie[n] {
        } else {
            panic!("n not pointing to a node when looking up a value");
        }
        loop {
            n += 1;
            match &trie[n] {
                TrieInstruction::Node(i, next) if *i == input => {
                    return Some(TrieCursor(self.0, *next));
                }
                TrieInstruction::Node(_, _) => {}
                TrieInstruction::Terminal(_) => {
                    return None;
                }
            }
        }
    }

    pub const fn get_val(&self) -> Option<&'a T> {
        let TrieCursor(Trie(trie), n) = self;
        if let TrieInstruction::Terminal(v) = &trie[*n] {
            v.as_ref()
        } else {
            None
        }
    }

    pub fn steps(&self, input: &[u8]) -> Option<TrieCursor<'a, T, N>> {
        let mut iptr = 0;
        let mut n = *self;
        while iptr < input.len() {
            if let Some(next) = n.step(input[iptr]) {
                n = next;
            } else {
                return None;
            }
            iptr += 1;
        }
        Some(n)
    }
    pub fn lookup_final(&self, input: &[u8]) -> Option<&'a T> {
        self.steps(input)?.get_val()
    }
}

impl<T: Copy + core::fmt::Debug, const N: usize> Trie<T, N> {
    pub fn lookup_total<'a>(&'a self, input: &[u8]) -> Option<&'a T> {
        self.start().lookup_final(input)
    }

    pub const fn start<'a>(&'a self) -> TrieCursor<'a, T, N> {
        TrieCursor(self, 0)
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn add_to_trie<'a>(
        self,
        mut alloc_idx: usize,
        k: usize,
        input: &[u8],
        value: Option<T>,
    ) -> (Self, usize)
    where
        [TrieInstruction<T>; N]: Copy,
    {
        let Trie(mut new_trie) = self;
        let mut ptr = 0;
        let mut iptr = 0;
        while iptr < input.len() {
            let i = input[iptr];
            iptr += 1;
            ptr += 1;
            'node_loop: loop {
                match &new_trie[ptr] {
                    TrieInstruction::Node(j, next) if *j == i => {
                        if let TrieInstruction::Terminal(_) = new_trie[*next] {
                            ptr = *next;
                            break 'node_loop;
                        } else {
                            panic!("Malformed trie input");
                        }
                    }
                    TrieInstruction::Node(_, _) => {
                        ptr += 1;
                    }
                    TrieInstruction::Terminal(_) => {
                        // If we've reached the terminal, then check that we still have space, and
                        // insert.
                        if let TrieInstruction::Terminal(None) = new_trie[ptr] {
                        } else {
                            panic!("Overflowed available per-node space in trie");
                        }
                        // println!("Moving terminal ahead");
                        // new_trie[ptr+1] = new_trie[ptr];
                        new_trie[ptr] = TrieInstruction::Node(i, alloc_idx);
                        ptr = alloc_idx; // Leave the Terminal(None) in place on alloc_idx
                        alloc_idx += k;
                        break 'node_loop;
                    }
                }
            }
        }
        // Now ptr is on a Terminal node for the whole input; fill it.
        if let TrieInstruction::Terminal(None) = new_trie[ptr] {
        } else {
            panic!("Tried to set value for key in trie twice.");
        }
        new_trie[ptr] = TrieInstruction::Terminal(value);
        (Trie(new_trie), alloc_idx)
    }

    // pub const fn pack_trie

    /*pub const fn build_trie<T: Copy, const M: usize>(&[(&[u8], T)]) -> [TrieInstruction<T>; M] where [TrieInstruction<T>; M]: Copy {

    }*/

    pub const fn build(pairs: &[(&[u8], T)]) -> Self {
        let offset = pairs.len() + 1;
        let mut current_trie = Trie([TrieInstruction::Terminal(None); N]);
        let mut alloc_idx = 3;
        let mut iptr = 0;
        while iptr < pairs.len() {
            let (i, v) = pairs[iptr];
            iptr += 1;
            (current_trie, alloc_idx) = current_trie.add_to_trie(alloc_idx, offset, i, Some(v));
        }
        current_trie
    }

    // Dumb implementation
    pub const fn repack(self) -> Self {
        let Trie(mut trie) = self;
        let mut i = 0; // index to the currently-under-consideration point in the trie
        let mut k = 0; // Extra counter to stop when we've cleared the whole item
        while i < N - 1 && k < N {
            i += 1;
            k += 1;
            if let TrieInstruction::Terminal(None) = trie[i] {
            } else {
                continue;
            }
            if let TrieInstruction::Terminal(_) = trie[i + 1] {
                // Now we know that trie[i] is redundant, remove it.
                let mut j = 0;
                while j < N {
                    if let TrieInstruction::Node(item, n) = trie[j] {
                        if n > i {
                            trie[j] = TrieInstruction::Node(item, n - 1);
                        }
                    }
                    j += 1;
                }
                let mut j = i;
                while j < N - 1 {
                    trie[j] = trie[j + 1];
                    j += 1;
                }
                // New instruction in trie[i], we should check it too
                i -= 1;
            }
        }
        Trie(trie)
    }

    pub const fn length(&self) -> usize {
        let Trie(ref trie) = self;
        let mut i = N - 1;
        while let TrieInstruction::Terminal(None) = trie[i] {
            if i == 0 {
                return 0;
            }
            i -= 1;
        }
        i + 1
    }

    pub const fn trim_to_len<const M: usize>(self) -> Trie<T, M> {
        // input[0..N].try_into().unwrap()
        let Trie(input) = self;
        let mut rv = [TrieInstruction::Terminal(None); M];
        let mut i = 0;
        while i < M {
            rv[i] = input[i];
            i += 1;
        }
        Trie(rv)
    }
}

pub const fn max_len<'a, T, const M: usize>(input: [(&[u8], T); M]) -> usize
where
    [(&'a [u8], T); M]: Copy,
{
    let mut i = 0;
    let mut max = 0;
    while i < M {
        if input[i].0.len() > max {
            max = input[i].0.len();
        }
        i += 1;
    }
    max
}

#[macro_export]
macro_rules! static_trie {
    { $name:ident <$t:ty> = $pairs:ident } =>
    { $crate::paste! {
        const [<$name _TEMP_TRIE>] : $crate::Trie<$t, { ($crate::max_len($pairs)+1)*$pairs.len()*($pairs.len()+1) }> = $crate::Trie::build(&$pairs).repack();
        const [<$name _TRIE_LEN>] : usize = [<$name _TEMP_TRIE>].length();
        static $name : $crate::Trie<$t, [<$name _TRIE_LEN>]> = [<$name _TEMP_TRIE>].trim_to_len();
        }
    }
}

pub trait TrieLookup: Copy {
    const N: usize;
    fn start() -> TrieCursor<'static, Self, { Self::N }>;
    fn lookup(i: &[u8]) -> Option<Self>;
}

/// A derive macro would be a cleaner approach here but more fuss.
#[macro_export]
macro_rules! enum_trie {
    { $name:ident { $($variant:ident = $str:literal),* } } =>
    { $crate::paste! {
        #[derive(Debug,Clone,Copy,PartialEq)]
        enum $name {
            $($variant),*
        }
        const [ <$name:snake:upper _PAIRS> ] : [ (&[u8], $name) ; { [$($name::$variant),*].len() }] = [ $(($str, $name::$variant)),* ];
        $crate::static_trie! { [< $name:snake:upper _TRIE >] < $name > = [ <$name:snake:upper _PAIRS> ] }
        impl TrieLookup for $name {
            const N : usize = [< $name:snake:upper _TRIE_TRIE_LEN >];
            #[allow(dead_code)]
            fn start() -> $crate::TrieCursor<'static, Self, [< $name:snake:upper _TRIE_TRIE_LEN >] > {
                [< $name:snake:upper _TRIE >].start()
            }
            #[allow(dead_code)]
            fn lookup(i: &[u8]) -> Option<Self> {
                [< $name:snake:upper _TRIE >].lookup_total(i).copied()
            }
        }
                    }
        /*impl $name {
            fn lookup([u8]) -> usize {
                for i in input {
                }
            }
            fn get_val() -> $name {
            }
            fn lookup_total() -> $name {
            }
        }*/
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    const SOME_PAIRS: [(&[u8], usize); 2] = [(b"Foo", 1), (b"Bar", 2)];
    static_trie! { STATIC_TRIE <usize> = SOME_PAIRS }

    const SOME_PAIRS2: [(&[u8], usize); 2] = [(b"Foo", 1), (b"Far", 2)];
    static_trie! { STATIC_TRIE2 <usize> = SOME_PAIRS2 }

    const SOME_PAIRS3: [(&[u8], usize); 2] = [(b"Foo", 1), (b"For", 2)];
    static_trie! { STATIC_TRIE3 <usize> = SOME_PAIRS3 }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum ExampleEnum {
        One,
        Two,
        Three,
        Twelve,
    }

    const EXAMPLE_PAIRS: [(&[u8], ExampleEnum); 4] = [
        (b"one", ExampleEnum::One),
        (b"two", ExampleEnum::Two),
        (b"three", ExampleEnum::Three),
        (b"twelve", ExampleEnum::Twelve),
    ];
    static_trie! { EXAMPLE_TRIE <ExampleEnum> = EXAMPLE_PAIRS }

    enum_trie! { Ex2 { A = b"Alph", B = b"BOO" } }

    #[test]
    fn it_works() {
        let some_trie: Trie<usize, 40> = Trie::build(&[(b"Foo", 1), (b"bar", 2)]);
        assert_eq!(some_trie.lookup_total(b"Foo"), Some(&1));
        assert_eq!(some_trie.lookup_total(b"bar"), Some(&2));
        assert_eq!(some_trie.lookup_total(b"baz"), None);
        assert_eq!(some_trie.lookup_total(b"quux"), None);
        println!("Trie: {:?}", some_trie);
        println!("Trie lookup: {:?}", some_trie.lookup_total(b"Foo"));

        let repacked = some_trie.repack();
        println!("Packed: {:?}", repacked);
        println!("Packed lookup: {:?}", repacked.lookup_total(b"Foo"));

        let other_trie = Trie::<_, 30>::build(&[(b"Foo", 1), (b"Far", 2)]).repack();
        println!("Overlapping: {:?}", other_trie);
        assert_eq!(other_trie.length(), 11);

        let other_trie = Trie::<_, 30>::build(&[(b"Foo", 1), (b"For", 2)]).repack();
        assert_eq!(other_trie.length(), 9);
        println!("Overlapping: {:?}", other_trie);

        println!("Static packed trie: {:?}", STATIC_TRIE);
        assert_eq!(STATIC_TRIE.0.len(), 13);

        println!("Static packed trie2: {:?}", STATIC_TRIE2);
        println!("Static packed trie2: {:?}", STATIC_TRIE2_TEMP_TRIE);
        assert_eq!(STATIC_TRIE2.0.len(), 11);

        println!("Static packed trie3: {:?}", STATIC_TRIE3);
        println!("Static packed trie3: {:?}", STATIC_TRIE3_TEMP_TRIE);
        assert_eq!(STATIC_TRIE3.0.len(), 9);

        assert_eq!(EXAMPLE_TRIE.lookup_total(b"one"), Some(&ExampleEnum::One));
        assert_eq!(EXAMPLE_TRIE.lookup_total(b"two"), Some(&ExampleEnum::Two));
        assert_eq!(
            EXAMPLE_TRIE.lookup_total(b"three"),
            Some(&ExampleEnum::Three)
        );
        assert_eq!(
            EXAMPLE_TRIE.lookup_total(b"twelve"),
            Some(&ExampleEnum::Twelve)
        );
        assert_eq!(EXAMPLE_TRIE.lookup_total(b"twenty"), None);
        assert_eq!(EXAMPLE_TRIE.lookup_total(b"twentyfiveten"), None);
        assert_eq!(EXAMPLE_TRIE.lookup_total(b"nine"), None);

        assert_eq!(EX2_TRIE.lookup_total(b"Alph"), Some(&Ex2::A));
        assert_eq!(EX2_TRIE.lookup_total(b"BOO"), Some(&Ex2::B));

        assert_eq!(Ex2::lookup(b"Alph"), Some(Ex2::A));
        assert_eq!(Ex2::lookup(b"BOO"), Some(Ex2::B));

        assert_eq!(
            Ex2::start()
                .steps(b"Al")
                .unwrap()
                .steps(b"ph")
                .unwrap()
                .get_val(),
            Some(&Ex2::A)
        );
        assert!(Ex2::start().steps(b"Al").unwrap().steps(b"phe").is_none());
    }
}
