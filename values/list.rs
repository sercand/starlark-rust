// Copyright 2018 The Starlark in Rust Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Define the list type of Starlark
use values::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::cmp::Ordering;
use std::borrow::BorrowMut;


pub struct List {
    frozen: bool,
    content: Vec<Value>,
}

impl<T: Into<Value> + Clone> From<Vec<T>> for List {
    fn from(a: Vec<T>) -> List {
        let mut result = List {
            frozen: false,
            content: Vec::new(),
        };
        for x in a.iter() {
            let v: Value = x.clone().into();
            result.content.push(v);
        }
        result
    }
}

impl List {
    pub fn new() -> Value {
        Value::new(List {
            frozen: false,
            content: Vec::new(),
        })
    }
}

impl TypedValue for List {
    fn immutable(&self) -> bool {
        self.frozen
    }
    fn freeze(&mut self) {
        self.frozen = true;
        for x in self.content.iter_mut() {
            x.borrow_mut().freeze();
        }
    }

    /// Returns a string representation for the list
    ///
    /// # Examples:
    /// ```
    /// # use starlark::values::*;
    /// # use starlark::values::list::List;
    /// assert_eq!("[1, 2, 3]", Value::from(vec![1, 2, 3]).to_str());
    /// assert_eq!("[1, [2, 3]]",
    ///            Value::from(vec![Value::from(1), Value::from(vec![2, 3])]).to_str());
    /// assert_eq!("[1]", Value::from(vec![1]).to_str());
    /// assert_eq!("[]", Value::from(Vec::<i64>::new()).to_str());
    /// ```
    fn to_str(&self) -> String {
        format!(
            "[{}]",
            self.content.iter().map(|x| x.to_repr()).enumerate().fold(
                "".to_string(),
                |accum, s| if s.0 == 0 {
                    accum + &s.1
                } else {
                    accum + ", " + &s.1

                },
            )
        )
    }

    fn to_repr(&self) -> String {
        self.to_str()
    }

    not_supported!(to_int);
    fn get_type(&self) -> &'static str {
        "list"
    }
    fn to_bool(&self) -> bool {
        !self.content.is_empty()
    }
    fn get_hash(&self) -> Result<u64, ValueError> {
        let mut s = DefaultHasher::new();
        for v in self.content.iter() {
            s.write_u64(v.get_hash()?)
        }
        Ok(s.finish())
    }

    fn compare(&self, other: Value) -> Ordering {
        if other.get_type() == "list" {
            let mut iter1 = self.into_iter().unwrap();
            let mut iter2 = other.into_iter().unwrap();
            loop {
                match (iter1.next(), iter2.next()) {
                    (None, None) => return Ordering::Equal,
                    (None, Some(..)) => return Ordering::Less,
                    (Some(..), None) => return Ordering::Greater,
                    (Some(v1), Some(v2)) => {
                        let r = v1.compare(v2);
                        if r != Ordering::Equal {
                            return r;
                        }
                    }
                }
            }
        } else {
            default_compare(self, other)
        }
    }

    fn at(&self, index: Value) -> ValueResult {
        let i = index.convert_index(self.length()?)? as usize;
        Ok(self.content[i].clone())
    }

    fn length(&self) -> Result<i64, ValueError> {
        Ok(self.content.len() as i64)
    }

    fn is_in(&self, other: Value) -> ValueResult {
        Ok(Value::new(self.content.iter().any(|x| {
            x.compare(other.clone()) == Ordering::Equal
        })))
    }

    fn slice(
        &self,
        start: Option<Value>,
        stop: Option<Value>,
        stride: Option<Value>,
    ) -> ValueResult {
        let (start, stop, stride) =
            Value::convert_slice_indices(self.length()?, start, stop, stride)?;
        let (low, take, astride) = if stride < 0 {
            (stop + 1, start - stop, -stride)
        } else {
            (start, stop - start, stride)
        };
        let mut v: Vec<Value> = self.content
            .iter()
            .skip(low as usize)
            .take(take as usize)
            .enumerate()
            .filter_map(|x| if 0 == (x.0 as i64 % astride) {
                Some(x.1.clone())
            } else {
                None
            })
            .collect();
        if stride < 0 {
            v.reverse();
        }
        Ok(Tuple::new(&v))
    }

    fn into_iter<'a>(&'a self) -> Result<Box<Iterator<Item = Value> + 'a>, ValueError> {
        Ok(Box::new(self.content.iter().map(|x| x.clone())))
    }

    /// Concatenate `other` to the current value.
    ///
    /// `other` has to be a list.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use starlark::values::*;
    /// # use starlark::values::list::List;
    /// # assert!(
    /// // [1, 2, 3] + [2, 3] == [1, 2, 3, 2, 3]
    /// Value::from(vec![1,2,3]).add(Value::from(vec![2,3])).unwrap()
    ///     == Value::from(vec![1, 2, 3, 2, 3])
    /// # );
    /// ```
    fn add(&self, other: Value) -> ValueResult {
        if other.get_type() == "list" {
            let mut result = List {
                frozen: false,
                content: Vec::new(),
            };
            for x in self.content.iter() {
                result.content.push(x.clone());
            }
            for x in other.into_iter()? {
                result.content.push(x.clone());
            }
            Ok(Value::new(result))
        } else {
            Err(ValueError::IncorrectParameterType)
        }
    }

    /// Repeat `other` times this tuple.
    ///
    /// `other` has to be an int or a boolean.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use starlark::values::*;
    /// # use starlark::values::list::List;
    /// # assert!(
    /// // [1, 2, 3] * 3 == [1, 2, 3, 1, 2, 3, 1, 2, 3]
    /// Value::from(vec![1,2,3]).mul(Value::from(3)).unwrap()
    ///              == Value::from(vec![1, 2, 3, 1, 2, 3, 1, 2, 3])
    /// # );
    /// ```
    fn mul(&self, other: Value) -> ValueResult {
        if other.get_type() == "int" || other.get_type() == "boolean" {
            let l = other.to_int()?;
            let mut result = List {
                frozen: false,
                content: Vec::new(),
            };
            for _i in 0..l {
                for x in self.content.iter() {
                    result.content.push(x.clone());
                }
            }
            Ok(Value::new(result))
        } else {
            Err(ValueError::IncorrectParameterType)
        }
    }

    /// Set the value at `index` to `new_value`
    ///
    /// # Example
    /// ```
    /// # use starlark::values::*;
    /// # use starlark::values::list::List;
    /// let mut v = Value::from(vec![1, 2, 3]);
    /// v.set_at(Value::from(1), Value::from(1)).unwrap();
    /// v.set_at(Value::from(2), Value::from(vec![2, 3])).unwrap();
    /// assert_eq!(&v.to_repr(), "[1, 1, [2, 3]]");
    /// ```
    fn set_at(&mut self, index: Value, new_value: Value) -> Result<(), ValueError> {
        if self.frozen {
            Err(ValueError::CannotMutateImmutableValue)
        } else {
            let i = index.convert_index(self.length()?)? as usize;
            self.content[i] = new_value.clone();
            Ok(())
        }
    }

    not_supported!(attr, function);
    not_supported!(plus, minus, sub, div, pipe, percent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_str() {
        assert_eq!("[1, 2, 3]", Value::from(vec![1, 2, 3]).to_str());
        assert_eq!(
            "[1, [2, 3]]",
            Value::from(vec![Value::from(1), Value::from(vec![2, 3])]).to_str()
        );
        assert_eq!("[1]", Value::from(vec![1]).to_str());
        assert_eq!("[]", Value::from(Vec::<i64>::new()).to_str());
    }

    #[test]
    fn test_mutate_list() {
        let mut v = Value::from(vec![1, 2, 3]);
        v.set_at(Value::from(1), Value::from(1)).unwrap();
        v.set_at(Value::from(2), Value::from(vec![2, 3])).unwrap();
        assert_eq!(&v.to_repr(), "[1, 1, [2, 3]]");
    }

    #[test]
    fn test_arithmetic_on_list() {
        // [1, 2, 3] + [2, 3] == [1, 2, 3, 2, 3]
        assert_eq!(
            Value::from(vec![1, 2, 3])
                .add(Value::from(vec![2, 3]))
                .unwrap(),
            Value::from(vec![1, 2, 3, 2, 3])
        );
        // [1, 2, 3] * 3 == [1, 2, 3, 1, 2, 3, 1, 2, 3]
        assert_eq!(
            Value::from(vec![1, 2, 3]).mul(Value::from(3)).unwrap(),
            Value::from(vec![1, 2, 3, 1, 2, 3, 1, 2, 3])
        );
    }
}
