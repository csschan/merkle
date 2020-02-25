# Merkle Tree implemented

###Storage format

A binary tree is stored in a vector in breadth-first order. That is, starting with the root we go from left to right at every level.

```
    1
  2   3
 4 5 6 7
 ```
 
 Vector:
 
 ```
 [1 2 3 4 5 6 7]
 ```
 
 While building a tree, if there is an odd number of nodes at the given level, the last node will be duplicated. Otherwise, the tree won't be complete. And we need it to be complete in order to "2i 2i+1" schema to work.
 
 ###Usage
 
 Let's say you have a file. You split it into 100 blocks and build a tree.
 
 ```
 use merkle_tree::MerkleTree;

let t: MerkleTree = MerkleTree::build(&blocks);
```

block could be anything, as long as it implements [AsBytes] trait. In order to encode the numbers, you can use byteorder library. If the block is an array of bytes, you don't have to do anything.

As we mentioned earlier, you can pass your hash function:

```
use merkle_tree::MerkleTree;

let t: MerkleTree = MerkleTree::build_with_hasher(&blocks, MyAwesomeHasher::new());
```

Then you somehow make a secure copy of the root hash.

```
t.root_hash();
```

You can now copy leaves from any source.

```
t.leaves();
```

If we verify that those leaves sum up to the root_hash, we can use them to verify the blocks. Blocks could be received and checked one by one.

```
let t: MerkleTree = MerkleTree::build_from_leaves(&leaves);
assert_eq!(secure_copy_of_root_hash, t.root_hash());

assert!(t.verify(block_index, &block));
```

where block_index - index of a block (starts at 0).

##Decision log

##Why binary tree?

None of the sources say anything about the number of children each node could have. The usual choice is 2. So, we need to know only the hash of our neighbor to check the subtree. We go all the way up to the root node. At each level, we only need to know the hash of our neighbor to the right (or left). This is of course if you want to check log(N) hashes on a path to root.
```
    1
  2   3
```

###Why tree is stored in a vector?

I've tried different solutions. At the end of the first day I had a standard binary tree (tag: 0.1.0).

```
struct Node
{
    hash: Vec<u8>,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}
```

Then I added an array of references to the lower elements to it (using Rc).

```
struct Node
{
    hash: Vec<u8>,
    left: Option<Rc<Node>>,
    right: Option<Rc<Node>>,
}

struct MerkleTree {
    root: Node,
    leaves: Vec<Rc<Node>>,
}
```

The main advantage of such tree is the ability to get the branches and check them in the absence of a full tree. BUT then I was faced with a contradiction in the article on Wikipedia: we can get the branches, but we don't trust the tree until it converges to the root hash. So what we should do? Get the branches and hope for the best?

Advantages of the final implementation

The ease of tree traversal
The absence of pointers in both directions (parent <-> child)
Ease of serialization - it's just an array
(1) to get the parent element, you need to divide the index of the current node in half: 5 / 2 = 2. That's it! The index of the left child - 2i, right child - 2i+1. This way we get the ease of traversing the tree from root to children and vice-versa (from children to root).

(3) to get all the leaves, we just need to get count_leaves last elements of the array.

Cons of the final implementation

Mathematics (2i, 2i+1) is still more complicated comparing to following the pointers: e.left.right.
The tree should be complete (except for the last level) for math to work. Sometimes we have to add duplicates.
Possible improvements

Provide implementations for AsBytes for a greater number of types.
In build_upper_level put nodes into the nodes (in-place) without creating intermediate arrays.
Deal with "rust cannot infer type for _" (let _t: MerkleTree).
Serialization/deserialization in a separate module.
User-friendly interface for traversing a tree (root().left().right()) - Builder pattern.

###Development

####Step 1. Create development environment
You will need docker engine if you want to develop this library inside a container. If you already have rust on your computer and fine with downloading some dependencies, feel free to skip this step.

```
$ make create_dev_env
$ make run_shell
```

Step 2. Build & test

```
$ cargo build --features "dev"
$ cargo test
```
Step 3. Benchmark

```
$ cargo bench --features "dev"
```
