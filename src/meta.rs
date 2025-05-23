use crate::*;

/// A simple map that can be used to store metadata
/// for the pagecache tenant.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Meta {
    pub(crate) inner: BTreeMap<IVec, PageId>,
}

impl Meta {
    /// Retrieve the `PageId` associated with an identifier
    pub(crate) fn get_root(&self, table: &[u8]) -> Option<PageId> {
        self.inner.get(table).cloned()
    }

    /// Set the `PageId` associated with an identifier
    pub(crate) fn set_root(
        &mut self,
        name: IVec,
        pid: PageId,
    ) -> Option<PageId> {
        self.inner.insert(name, pid)
    }

    /// Remove the page mapping for a given identifier
    pub(crate) fn del_root(&mut self, name: &[u8]) -> Option<PageId> {
        self.inner.remove(name)
    }
}

/// Open or create a new disk-backed Tree with its own keyspace,
/// accessible from the `Db` via the provided identifier.
pub(crate) fn open_tree<V>(
    context: &Context,
    raw_name: V,
    guard: &Guard,
) -> Result<Tree>
where
    V: Into<IVec>,
{
    let name = raw_name.into();

    // we loop because creating this Tree may race with
    // concurrent attempts to open the same one.
    loop {
        match context.pagecache.meta_pid_for_name(&name, guard) {
            Ok(root_id) => {
                assert_ne!(root_id, 0);
                return Ok(Tree(Arc::new(TreeInner {
                    tree_id: name,
                    context: context.clone(),
                    subscribers: Subscribers::default(),
                    root: AtomicU64::new(root_id),
                    merge_operator: RwLock::new(None),
                })));
            }
            Err(Error::CollectionNotFound) => {}
            Err(other) => return Err(other),
        }

        // set up empty leaf
        let leaf = Node::new_empty_leaf();
        let (leaf_id, leaf_ptr) = context.pagecache.allocate(leaf, guard)?;

        trace!(
            "allocated pid {} for leaf in new_tree for namespace {:?}",
            leaf_id, name
        );

        // set up root index

        // vec![0] represents a prefix-encoded empty prefix
        let root = Node::new_root(leaf_id);
        let (root_id, root_ptr) = context.pagecache.allocate(root, guard)?;

        debug!("allocated pid {} for root of new_tree {:?}", root_id, name);

        let res = context.pagecache.cas_root_in_meta(
            &name,
            None,
            Some(root_id),
            guard,
        )?;

        if res.is_err() {
            // clean up the tree we just created if we couldn't
            // install it.
            let _ = context
                .pagecache
                .free(root_id, root_ptr, guard)?
                .expect("could not free allocated page");
            let _ = context
                .pagecache
                .free(leaf_id, leaf_ptr, guard)?
                .expect("could not free allocated page");
            continue;
        }

        return Ok(Tree(Arc::new(TreeInner {
            tree_id: name,
            subscribers: Subscribers::default(),
            context: context.clone(),
            root: AtomicU64::new(root_id),
            merge_operator: RwLock::new(None),
        })));
    }
}
