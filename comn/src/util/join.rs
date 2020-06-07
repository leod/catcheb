use std::iter::Peekable;

pub fn full_join<Left, Right, K, T, U>(left: Left, right: Right) -> FullJoinIter<Left, Right>
where
    Left: Iterator<Item = (K, T)>,
    Right: Iterator<Item = (K, U)>,
    K: Ord,
{
    FullJoinIter {
        left: left.peekable(),
        right: right.peekable(),
    }
}

/// Element of a full join
pub enum Item<K, T, U> {
    /// The key `K` is only contained in the left iterator
    Left(K, T),

    /// The key `K` is only contained in the right iterator
    Right(K, U),

    /// The key `K` is contained in both iterators
    Both(K, T, U),
}

/// Iterator over the full join of two sequences of key value pairs. The
/// sequences are assumed to be sorted by the key in ascending order.
pub struct FullJoinIter<Left, Right>
where
    Left: Iterator,
    Right: Iterator,
{
    left: Peekable<Left>,
    right: Peekable<Right>,
}

impl<Left, Right, K, T, U> Iterator for FullJoinIter<Left, Right>
where
    Left: Iterator<Item = (K, T)>,
    Right: Iterator<Item = (K, U)>,
    K: Ord,
{
    type Item = Item<K, T, U>;

    fn next(&mut self) -> Option<Self::Item> {
        // Advance the iterator which has the element with the smaller key.
        match (self.left.peek(), self.right.peek()) {
            (Some((left_k, _)), Some((right_k, _))) => Some(if left_k < right_k {
                let (left_k, left_v) = self.left.next().unwrap();
                Item::Left(left_k, left_v)
            } else if left_k > right_k {
                let (right_k, right_v) = self.right.next().unwrap();
                Item::Right(right_k, right_v)
            } else {
                let (left_k, left_v) = self.left.next().unwrap();
                let (right_k, right_v) = self.right.next().unwrap();
                assert!(left_k == right_k);
                Item::Both(left_k, left_v, right_v)
            }),
            (Some(_), None) => {
                let (left_k, left_v) = self.left.next().unwrap();
                Some(Item::Left(left_k, left_v))
            }
            (None, Some(_)) => {
                let (right_k, right_v) = self.right.next().unwrap();
                Some(Item::Right(right_k, right_v))
            }
            (None, None) => None,
        }
    }
}
