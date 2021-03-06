#![deny(missing_docs,
    missing_debug_implementations, missing_copy_implementations,
    trivial_casts, trivial_numeric_casts,
    unsafe_code, 
    unstable_features,
    unused_import_braces, unused_qualifications)]

#![cfg_attr(feature = "dev", allow(unstable_features))]
#![cfg_attr(feature = "dev", feature(plugin))]
#![cfg_attr(feature = "dev",feature(test))]
#![cfg_attr(feature = "dev", plugin(clippy))]

//! Merkle Tree implementation
extern crate test;
extern crate crypto;
extern crate rustc_serialize;

mod bench;
mod utils;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use std::fmt;

const LEAF_SIG: u8 = 0u8;
const INTERNAL_SIG: u8 = 1u8;

type Hash = Vec<u8>;

/// Merkle Tree is a binary tree, which nodes values are the hash of the
/// concatenated values of their descendants hashes.
///
/// [Main article on Wikipedia](https://en.wikipedia.org/wiki/Merkle_tree)
///
/// # Storage Format
///
/// A binary tree is stored in a vector in breadth-first order. That is,
/// starting with the root we go from left to right at every level.
///
/// ```text
///     1
///   2   3
///  4 5 6 7
/// ```
///
/// Vector:
///
/// ```text
/// [1 2 3 4 5 6 7]
/// ```
///
/// While building a tree, if there is an odd number of nodes at the given
/// level, the last node will be duplicated. Otherwise, the tree won't be
/// complete. And we need it to be complete in order to "2i 2i+1" schema to
/// work.
///
/// # Defence against potential attacks
///
/// To defend against the second-preimage attack, when we calculate the hash we
/// prepend data with 0x00 - for leaves, 0x01 - for internal nodes.
///
/// By default, we use SHA256. But you can pass your hash function (for example, double SHA256).
///
/// # Usage
///
/// Let's say you have a file. You split it into 100 blocks and build a tree.
///
/// ```text
/// use merkle_tree::MerkleTree;
///
/// let t: MerkleTree = MerkleTree::build(&blocks);
/// ```
///
/// block could be anything, as long as it implements [`AsBytes`] trait. In
/// order to encode the numbers, you can use [byteorder
/// library](https://github.com/BurntSushi/byteorder). If the block is an array
/// of bytes, you don't have to do anything.
///
/// As we mentioned earlier, you can pass your hash function:
///
/// ```text
/// use merkle_tree::MerkleTree;
///
/// let t: MerkleTree = MerkleTree::build_with_hasher(&blocks, MyAwesomeHasher::new());
/// ```
///
/// Then you somehow make a secure copy of the root hash.
///
/// ```text
/// t.root_hash();
/// ```
///
/// You can now copy leaves from any source.
///
/// ```text
/// t.leaves();
/// ```
///
/// If we verify that those leaves sum up to the `root_hash`, we can use them to
/// verify the blocks. Blocks could be received and checked one by one.
///
/// ```text
/// let t: MerkleTree = MerkleTree::build_from_leaves(&leaves);
/// assert_eq!(secure_copy_of_root_hash, t.root_hash());
///
/// assert!(t.verify(block_index, &block));
/// ```
///
/// where `block_index` - index of a block (starts at 0).
#[derive(Debug)]
pub struct MerkleTree<H = DefaultHasher> {
hasher: H,
nodes: Vec<Hash>,
count_internal_nodes: usize,
count_leaves: usize,
}

fn hash_leaf<T, H>(value: &T, hasher: &mut H) -> Hash
where T: AsBytes,
      H: Digest
{
let mut result = vec![0u8; hasher.output_bits() / 8];

hasher.reset();
hasher.input(&[LEAF_SIG]);
hasher.input(value.as_bytes());
hasher.result(result.as_mut_slice());

result
}

fn hash_internal_node<H>(left: &Hash, right: Option<&Hash>, hasher: &mut H) -> Hash
where H: Digest
{
let mut result = vec![0u8; hasher.output_bits() / 8];

hasher.reset();
hasher.input(&[INTERNAL_SIG]);
hasher.input(left.as_slice());
if let Some(r) = right {
    hasher.input(r.as_slice());
} else {
    // if there is no right node, we hash left with itself
    hasher.input(left.as_slice());
}
hasher.result(result.as_mut_slice());

result
}

fn build_upper_level<H>(nodes: &[Hash], hasher: &mut H) -> Vec<Hash>
where H: Digest
{
let mut row = Vec::with_capacity((nodes.len() + 1) / 2);
let mut i = 0;
while i < nodes.len() {
    if i + 1 < nodes.len() {
        row.push(hash_internal_node(&nodes[i], Some(&nodes[i + 1]), hasher));
        i += 2;
    } else {
        row.push(hash_internal_node(&nodes[i], None, hasher));
        i += 1;
    }
}

if row.len() > 1 && row.len() % 2 != 0 {
    let last_node = row.last().unwrap().clone();
    row.push(last_node);
}

row
}

fn build_internal_nodes<H>(nodes: &mut Vec<Hash>, count_internal_nodes: usize, hasher: &mut H)
where H: Digest
{
let mut parents = build_upper_level(&nodes[count_internal_nodes..], hasher);

let mut upper_level_start = count_internal_nodes - parents.len();
let mut upper_level_end = upper_level_start + parents.len();
nodes[upper_level_start..upper_level_end].clone_from_slice(&parents);

while parents.len() > 1 {
    parents = build_upper_level(parents.as_slice(), hasher);

    upper_level_start -= parents.len();
    upper_level_end = upper_level_start + parents.len();
    nodes[upper_level_start..upper_level_end].clone_from_slice(&parents);
}

nodes[0] = parents.remove(0);
}

fn calculate_internal_nodes_count(count_leaves: usize) -> usize {
utils::next_power_of_2(count_leaves) - 1
}

fn _build_from_leaves_with_hasher<H>(leaves: &[Hash], mut hasher: H) -> MerkleTree<H>
where H: Digest
{
let count_leaves = leaves.len();
let count_internal_nodes = calculate_internal_nodes_count(count_leaves);
let mut nodes = vec![Vec::new(); count_internal_nodes + count_leaves];

// copy leafs
nodes[count_internal_nodes..].clone_from_slice(leaves);

build_internal_nodes(&mut nodes, count_internal_nodes, &mut hasher);

MerkleTree {
    nodes: nodes,
    count_internal_nodes: count_internal_nodes,
    count_leaves: count_leaves,
    hasher: hasher,
}
}

impl<H> MerkleTree<H> {
/// Constructs a tree from values of data. Data could be anything as long as it could be
/// represented as bytes array.
///
/// # Examples
///
/// ```
/// use merkle_tree::MerkleTree;
///
/// let block = "Hello World";
/// let _t: MerkleTree = MerkleTree::build(&[block, block]);
/// ```
pub fn build<T>(values: &[T]) -> MerkleTree<H>
    where H: Digest + Default,
          T: AsBytes
{
    let hasher = Default::default();
    MerkleTree::build_with_hasher(values, hasher)
}

/// Constructs a tree from values of data. Data could be anything as long as it could be
/// represented as bytes array.
///
/// Hasher could be any object, which implements `crypto::digest::Digest` trait. You could
/// write your own hasher if you want specific behaviour (e.g. double SHA256).
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate crypto;
/// # #[macro_use] extern crate merkle_tree;
/// # fn main() {
///     use merkle_tree::MerkleTree;
///     use crypto::sha2::Sha512;
///     type MT = MerkleTree<Sha512>;
///
///     let block = "Hello World";
///     let _t: MT = MT::build_with_hasher(&[block, block], Sha512::new());
/// }
/// ```
pub fn build_with_hasher<T>(values: &[T], mut hasher: H) -> MerkleTree<H>
    where H: Digest,
          T: AsBytes
{
    let count_leaves = values.len();
    assert!(count_leaves > 1,
            format!("expected more then 1 value, received {}", count_leaves));

    let leaves: Vec<Hash> = values.iter().map(|v| hash_leaf(v, &mut hasher)).collect();

    _build_from_leaves_with_hasher(leaves.as_slice(), hasher)
}

/// Constructs a tree from its leaves.
///
/// # Examples
///
/// ```
/// use merkle_tree::MerkleTree;
///
/// let block = "Hello World";
/// let t1: MerkleTree = MerkleTree::build(&[block, block]);
///
/// let t2: MerkleTree = MerkleTree::build_from_leaves(t1.leaves());
///
/// assert_eq!(t1.root_hash(), t2.root_hash());
/// ```
pub fn build_from_leaves(leaves: &[Hash]) -> MerkleTree<H>
    where H: Digest + Default
{
    let hasher = Default::default();
    MerkleTree::build_from_leaves_with_hasher(leaves, hasher)
}

/// Constructs a tree from its leaves.
///
/// Hasher could be any object, which implements `crypto::digest::Digest` trait. You could
/// write your own hasher if you want specific behaviour (e.g. double SHA256).
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate crypto;
/// # #[macro_use] extern crate merkle_tree;
/// # fn main() {
///     use merkle_tree::MerkleTree;
///     use crypto::sha2::Sha512;
///     type MT = MerkleTree<Sha512>;
///
///     let block = "Hello World";
///     let t1: MT = MT::build_with_hasher(&[block, block], Sha512::new());
///
///     let t2: MT = MT::build_from_leaves_with_hasher(t1.leaves(), Sha512::new());
///
///     assert_eq!(t1.root_hash(), t2.root_hash());
/// }
/// ```
pub fn build_from_leaves_with_hasher(leaves: &[Hash], hasher: H) -> MerkleTree<H>
    where H: Digest
{
    let count_leaves = leaves.len();
    assert!(count_leaves > 1,
            format!("expected more then 1 leaf, received {}", count_leaves));

    _build_from_leaves_with_hasher(leaves, hasher)
}

/// Returns the root hash of the tree.
///
/// # Examples
///
/// ```
/// use merkle_tree::MerkleTree;
///
/// let block = "Hello World";
/// let t: MerkleTree = MerkleTree::build(&[block, block]);
///
/// assert!(t.root_hash().len() > 0);
/// ```
pub fn root_hash(&self) -> &Hash {
    &self.nodes[0]
}

/// Returns root hash of the tree as a string.
///
/// # Examples
///
/// ```
/// use merkle_tree::MerkleTree;
///
/// let block = "Hello World";
/// let t: MerkleTree = MerkleTree::build(&[block, block]);
///
/// assert_ne!("", t.root_hash_str());
/// ```
pub fn root_hash_str(&self) -> String {
    use rustc_serialize::hex::ToHex;
    self.nodes[0].as_slice().to_hex()
}

/// Returns the leaves of the tree.
///
/// # Examples
///
/// ```
/// use merkle_tree::MerkleTree;
///
/// let block = "Hello World";
/// let t: MerkleTree = MerkleTree::build(&[block, block]);
///
/// assert_eq!(2, t.leaves().len());
/// ```
pub fn leaves(&self) -> &[Hash] {
    &self.nodes[self.count_internal_nodes..]
}

/// Verify value by comparing its hash against the one in the tree. `position` must not
/// exceed count of leaves and starts at 0.
///
/// # Examples
///
/// ```
/// use merkle_tree::MerkleTree;
///
/// let block1 = "Hello World";
/// let block2 = "Bye, bye";
/// let mut t: MerkleTree = MerkleTree::build(&[block1, block2]);
///
/// assert!(t.verify(0, &block1));
/// assert!(!t.verify(0, &block2));
/// ```
pub fn verify<T>(&mut self, position: usize, value: &T) -> bool
    where H: Digest,
          T: AsBytes
{
    assert!(position < self.count_leaves,
            "position does not relate to any leaf");

    self.nodes[self.count_internal_nodes + position].as_slice() ==
    hash_leaf(value, &mut self.hasher).as_slice()
}
}

/// The default [`Hasher`] used by [`MerkleTree`].
#[derive(Copy, Clone)]
pub struct DefaultHasher(Sha256);

impl DefaultHasher {
/// Creates a new `DefaultHasher`.
pub fn new() -> DefaultHasher {
    DefaultHasher(Sha256::new())
}
}

/// Implementation of the `Default` trait from std library
impl Default for DefaultHasher {
/// Creates a new `DefaultHasher` using [`DefaultHasher::new`]. See
/// [`DefaultHasher::new`] documentation for more information.
///
/// [`DefaultHasher::new`]: #method.new
fn default() -> DefaultHasher {
    DefaultHasher::new()
}
}

/// Implementation of the `Digest` trait from crypto library for our [`DefaultHasher`]
impl Digest for DefaultHasher {
#[inline]
fn input(&mut self, d: &[u8]) {
    self.0.input(d)
}

#[inline]
fn result(&mut self, out: &mut [u8]) {
    self.0.result(out)
}

#[inline]
fn reset(&mut self) {
    self.0.reset()
}

#[inline]
fn output_bits(&self) -> usize {
    self.0.output_bits()
}

#[inline]
fn block_size(&self) -> usize {
    self.0.block_size()
}
}

impl fmt::Debug for DefaultHasher {
fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    // Sha256 does not implement Display or Debug traits
    write!(f, "DefaultHasher {{ Sha256 }}")
}
}

/// [`AsBytes`] is implemeted for types which are given as values to
/// [`MerkleTree::build`] method.
pub trait AsBytes {
/// Converts value into the byte slice.
fn as_bytes(&self) -> &[u8];
}

impl<'a> AsBytes for &'a str {
fn as_bytes(&self) -> &[u8] {
    str::as_bytes(self)
}
}

impl AsBytes for String {
fn as_bytes(&self) -> &[u8] {
    String::as_bytes(self)
}
}

impl<'a> AsBytes for &'a [u8] {
fn as_bytes(&self) -> &[u8] {
    *self
}
}

#[cfg(test)]
mod tests {
use super::MerkleTree;

#[test]
#[should_panic]
fn test_build_with_0_values() {
    let _t: MerkleTree = MerkleTree::build::<String>(&[]);
}

#[test]
fn test_build_with_odd_number_of_values() {
    let block = "Hello World";
    let _t: MerkleTree = MerkleTree::build(&[block, block, block]);
}

#[test]
fn test_build_with_even_number_of_values() {
    let block = "Hello World";
    let _t: MerkleTree = MerkleTree::build(&[block, block, block, block]);
}

#[test]
fn test_root_hash_stays_the_same_if_data_hasnt_been_changed() {
    let block = "Hello World";
    let t: MerkleTree = MerkleTree::build(&[block, block]);

    assert_eq!("c9978dc3e2d729207ca4c012de993423f19e7bf02161f7f95cdbf28d1b57b88a",
               t.root_hash_str());
}

#[test]
fn test_root_children_have_the_same_hash_if_blocks_were_the_same() {
    let block = "Hello World";
    let t: MerkleTree = MerkleTree::build(&[block, block, block, block, block]);

    assert_eq!(t.nodes[1].as_slice(), t.nodes[2].as_slice());
}

#[test]
fn test_root_children_have_the_different_hash_if_blocks_were_the_different() {
    let block1 = "Hello World";
    let block2 = "Bye Bye";
    let t: MerkleTree = MerkleTree::build(&[block1, block1, block2, block2]);

    assert_ne!(t.nodes[1].as_slice(), t.nodes[2].as_slice());
}

#[test]
#[should_panic]
fn test_build_from_leaves_with_0_values() {
    let _t: MerkleTree = MerkleTree::build_from_leaves(&[]);
}

#[test]
fn test_building_a_tree_from_existing_tree() {
    let block = "Hello World";
    let existing_tree: MerkleTree = MerkleTree::build(&[block, block]);

    let new_tree: MerkleTree = MerkleTree::build_from_leaves(existing_tree.leaves());

    assert_eq!(new_tree.root_hash_str(), existing_tree.root_hash_str());
    assert_eq!(new_tree.leaves().len(), existing_tree.leaves().len());
    assert_eq!(new_tree.leaves(), existing_tree.leaves());
}
}
