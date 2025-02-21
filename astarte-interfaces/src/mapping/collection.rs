// This file is part of Astarte.
//
// Copyright 2023 SECO Mind Srl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

//! Sorted collection of interfaces accessible by endpoint or path.

use std::{fmt::Debug, slice::Iter as SliceIter};

use crate::interface::MAX_INTERFACE_MAPPINGS;

use super::{endpoint::Endpoint, path::MappingPath, InterfaceMapping, MappingError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MappingVec<T>
where
    T: InterfaceMapping,
{
    items: Vec<T>,
}

impl<T> MappingVec<T>
where
    T: InterfaceMapping,
{
    /// Create an empty vector.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Gets the mapping searching the [`Endpoint`]'s for the one matching the path.
    pub fn get(&self, path: &MappingPath<'_>) -> Option<&T>
    where
        T: InterfaceMapping + Debug,
    {
        self.items
            .binary_search_by(|item| {
                let endpoint = item.endpoint();

                endpoint.cmp_mapping(path)
            })
            .ok()
            .and_then(|idx| self.items.get(idx))
    }

    /// Gets the mapping searching the [`Endpoint`]'s for the one matching the path.
    pub fn get_by_endpoint(&self, endpoint: &Endpoint<String>) -> Option<&T>
    where
        T: InterfaceMapping + Debug,
    {
        self.items
            .binary_search_by(|item| item.endpoint().cmp(endpoint))
            .ok()
            .and_then(|idx| self.items.get(idx))
    }

    /// Returns the number of mappings.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns the number of mappings.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a MappingVec<T>
where
    T: InterfaceMapping,
{
    type Item = &'a T;

    type IntoIter = SliceIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<T> TryFrom<Vec<T>> for MappingVec<T>
where
    T: InterfaceMapping,
{
    type Error = MappingError;

    fn try_from(mut value: Vec<T>) -> Result<Self, Self::Error> {
        if value.len() == 0 {
            return Err(MappingError::Empty);
        }

        if value.len() > MAX_INTERFACE_MAPPINGS {
            return Err(MappingError::TooMany(value.len()));
        }

        value.sort_by(|a, b| a.endpoint().cmp(b.endpoint()));

        value
            .windows(2)
            .map(|window| {
                let [a, b] = window else {
                    // this never happen, for value.len < 2 the all is never called, see `windows`
                    unreachable!("windows returned less than 2 elements");
                };

                if a.endpoint() == b.endpoint() {
                    return Err(MappingError::Duplicated {
                        endpoint: a.endpoint().to_string(),
                    });
                }

                Ok(())
            })
            .collect::<Result<(), MappingError>>()?;

        Ok(Self { items: value })
    }
}

#[cfg(test)]
mod tests {
    use crate::mapping::endpoint::Endpoint;

    use super::*;

    fn make_vecs<'a>(cases: &'a [&str]) -> Vec<Endpoint> {
        let mut endpoints: Vec<Endpoint> = cases
            .iter()
            .map(|&s| Endpoint::try_from(s).unwrap())
            .collect();
        endpoints.sort();

        endpoints
    }

    #[test]
    fn should_get_mapping() {
        let cases = ["/foo/32", "/foo/bar", "/%{param}/param"];

        let endpoints = make_vecs(&cases);

        let mappings = MappingVec::try_from(endpoints.clone()).unwrap();

        for (case, endpoint) in cases.iter().zip(endpoints) {
            let path = MappingPath::try_from(*case).unwrap();
            let e = mappings
                .get(&path)
                .expect(&format!("couldn't get mapping {case}"));

            assert_eq!(*e, endpoint);
        }
    }

    #[test]
    fn should_iter_mappings() {
        // Order is important
        let cases = ["/foo/32", "/foo/bar", "/%{param}/param"];

        let endpoints = make_vecs(&cases);

        let mappings = MappingVec::try_from(endpoints.clone()).unwrap();

        for endpoint in endpoints {
            let e = mappings
                .get_by_endpoint(&endpoint)
                .expect(&format!("couldn't get endpoint {endpoint}"));

            assert_eq!(*e, endpoint);
        }
    }

    #[test]
    fn try_from_single_element() {
        // Order is important
        let cases = ["/foo/32"];

        let endpoints = make_vecs(&cases);

        MappingVec::try_from(endpoints).unwrap();
    }
}
