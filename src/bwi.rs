use std::iter::Step;

// BoundedWrappingIterator (it's not the most elegant, but it works)
pub struct BWI<T: Copy + Ord + Step> {
    first: Option<T>,
    current: Option<T>,
    to: T,
    last: Option<T>,
}

impl<T: Copy + Ord + Step> BWI<T> {
    // All values are inclusive
    pub fn new(from: T, to: T, lowest: T, highest: T, wrap: bool) -> Self {
        Self {
            first: if to > highest && wrap { Some(lowest.clone()) } else { None },
            last: if from < lowest && wrap { Some(highest.clone()) } else { None },
            current: if from > lowest { Some(from.clone()) } else { Some(lowest.clone()) },
            to: if to < highest { to } else { highest },
        }
    }
}

impl<T: Copy + Ord + Step> Iterator for BWI<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if let Some(first) = self.first {
            self.first = None;
            Some(first)
        } else if let Some(mid) = self.current {
            if T::forward(mid, 1) <= self.to {
                self.current = Some(T::forward(mid, 1))
            } else {
                self.current = None
            };
            Some(mid)
        } else if let Some(last) = self.last {
            self.last = None;
            Some(last)
        } else {
            None
        }
    }
}
