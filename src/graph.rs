use std::collections::HashMap;

use anyhow::Context;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeIdent(String);

impl From<&str> for NodeIdent {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for NodeIdent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default)]
pub struct NodeMonad {
    pub output: Option<String>,
}

impl NodeMonad {
    pub fn new<T>(output: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            output: Some(output.into()),
        }
    }
}

pub enum Move {
    Next { monad: NodeMonad, next: NodeIdent },
    Terminate,
}

pub trait INode {
    /// Try to step the node.
    /// If the node is the terminate state, return `Move::End`.
    /// Otherwise, return `Move::Next` with the name of the next node to step.
    fn step(&mut self, last: Option<NodeMonad>) -> anyhow::Result<Move>;
}

pub type DynNode = Box<dyn INode + Send>;

struct Edge {
    src: NodeIdent,
    dst: NodeIdent,
}

impl Edge {
    pub fn new<I, J>(src: I, dst: J) -> Self
    where
        I: Into<NodeIdent>,
        J: Into<NodeIdent>,
    {
        Self {
            src: src.into(),
            dst: dst.into(),
        }
    }

    pub fn src(&self) -> &NodeIdent {
        &self.src
    }

    pub fn dst(&self) -> &NodeIdent {
        &self.dst
    }
}

pub struct GraphBuilder {
    nodes: Vec<(NodeIdent, DynNode)>,
    edges: Vec<Edge>,
    start: NodeIdent,
}

impl GraphBuilder {
    pub fn new<I, N>(ident: I, start: N) -> Self
    where
        I: Into<NodeIdent>,
        N: INode + Send + 'static,
    {
        let ident = ident.into();
        Self {
            nodes: vec![(ident.clone(), Box::new(start))],
            edges: Vec::new(),
            start: ident,
        }
    }

    pub fn node<I, N>(mut self, name: I, node: N) -> Self
    where
        I: Into<NodeIdent>,
        N: INode + Send + 'static,
    {
        self.nodes.push((name.into(), Box::new(node)));
        self
    }

    pub fn edge<I>(mut self, src: I, dst: I) -> Self
    where
        I: Into<NodeIdent>,
    {
        self.edges.push(Edge::new(src, dst));
        self
    }

    pub fn build(self) -> anyhow::Result<Graph> {
        let mut nodes: HashMap<NodeIdent, DynNode> = HashMap::new();
        for (name, node) in self.nodes {
            if nodes.contains_key(&name) {
                anyhow::bail!("duplicate node: {:?}", name);
            }
            nodes.insert(name, node);
        }

        let mut edges: HashMap<NodeIdent, Vec<NodeIdent>> = HashMap::new();
        for edge in &self.edges {
            if !nodes.contains_key(edge.src()) {
                anyhow::bail!("edge source node not found: {:?}", edge.src());
            }
            if !nodes.contains_key(edge.dst()) {
                anyhow::bail!("edge target node not found: {:?}", edge.dst());
            }
            edges
                .entry(edge.src().clone())
                .or_default()
                .push(edge.dst().clone());
        }

        let start = self.start;

        anyhow::ensure!(
            nodes.contains_key(&start),
            "start node {:?} not found — this is a bug",
            start,
        );

        Ok(Graph {
            nodes,
            edges,
            start,
        })
    }
}

pub struct Graph {
    /// Map from node name to node instance.
    nodes: HashMap<NodeIdent, DynNode>,
    /// Map from node name to the list of next node names.
    edges: HashMap<NodeIdent, Vec<NodeIdent>>,

    /// A start node is required to have a well-defined entry point.
    start: NodeIdent,
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph").field("edges", &self.edges).finish()
    }
}

impl Graph {
    pub async fn run(&mut self) -> anyhow::Result<NodeMonad> {
        let mut last = None;
        let mut curr = self.start.clone();

        let mut rounds: usize = 0;
        loop {
            rounds += 1;

            let node = self
                .nodes
                .get_mut(&curr)
                .ok_or_else(|| anyhow::anyhow!("node not found: {:?}", curr))?;

            match node
                .step(last)
                .with_context(|| format!("step {} on node {:?}", rounds, curr))?
            {
                Move::Terminate => return Ok(NodeMonad::default()),
                Move::Next { monad, next } => last = self.next(&mut curr, monad, &next).await?,
            }
        }
    }

    async fn next(
        &self,
        curr: &mut NodeIdent,
        monad: NodeMonad,
        dst: &NodeIdent,
    ) -> anyhow::Result<Option<NodeMonad>> {
        let targets = self
            .edges
            .get(curr)
            .ok_or_else(|| anyhow::anyhow!("no edges from: {:?}", curr))?;

        anyhow::ensure!(
            targets.contains(dst),
            "invalid transition: {:?} -> {:?}. allowed: {:?}",
            curr,
            dst,
            targets,
        );

        *curr = dst.clone();

        Ok(Some(monad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node that echoes its input and always moves to a fixed next node.
    struct EchoNode {
        next: NodeIdent,
        /// If true, append this suffix to the raw output.
        suffix: &'static str,
        /// Terminate after this many steps (None = never).
        terminate_after: Option<usize>,
        steps: usize,
    }

    impl EchoNode {
        fn new<I>(next: I) -> Self
        where
            I: Into<NodeIdent>,
        {
            Self {
                next: next.into(),
                suffix: "",
                terminate_after: None,
                steps: 0,
            }
        }

        fn with_suffix(mut self, suffix: &'static str) -> Self {
            self.suffix = suffix;
            self
        }

        fn terminate_after(mut self, n: usize) -> Self {
            self.terminate_after = Some(n);
            self
        }
    }

    impl INode for EchoNode {
        fn step(&mut self, last: Option<NodeMonad>) -> anyhow::Result<Move> {
            self.steps += 1;

            if self.terminate_after == Some(self.steps) {
                return Ok(Move::Terminate);
            }

            let base = match last.and_then(|m| m.output) {
                Some(s) => s,
                None => "start".to_string(),
            };
            let raw = format!("{}{}", base, self.suffix);

            Ok(Move::Next {
                monad: NodeMonad::new(raw),
                next: self.next.clone(),
            })
        }
    }

    /// A node that always terminates immediately.
    struct TerminateNode;

    impl INode for TerminateNode {
        fn step(&mut self, _last: Option<NodeMonad>) -> anyhow::Result<Move> {
            Ok(Move::Terminate)
        }
    }

    // ── build tests ──

    #[test]
    fn build_linear_graph() {
        let graph = GraphBuilder::new("START", EchoNode::new("b"))
            .node("b", TerminateNode)
            .edge("START", "b")
            .build();

        assert!(
            graph.is_ok(),
            "linear graph should build: {:?}",
            graph.err()
        );
    }

    #[test]
    fn build_edge_to_missing_node() {
        // Edge to "END" — "END" is not a registered node, should fail.
        let result = GraphBuilder::new("START", EchoNode::new("END"))
            .edge("START", "END")
            .build();

        assert!(result.is_err(), "edge to unregistered END node should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("END"),
            "error should mention END, got: {}",
            err
        );
    }

    #[test]
    fn build_missing_src_node() {
        let result = GraphBuilder::new("START", TerminateNode)
            .edge("ghost", "START")
            .build();

        assert!(result.is_err(), "edge from unregistered node should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("ghost"),
            "error should mention ghost, got: {}",
            err
        );
    }

    #[test]
    fn build_duplicate_node() {
        let result = GraphBuilder::new("START", TerminateNode)
            .node("START", TerminateNode)
            .build();

        assert!(result.is_err(), "duplicate START node should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("duplicate"),
            "error should mention duplicate, got: {}",
            err
        );
    }

    // ── run tests ──

    #[tokio::test]
    async fn run_single_node_terminates() {
        let mut graph = GraphBuilder::new("START", TerminateNode).build().unwrap();
        let result = graph.run().await;

        assert!(
            result.is_ok(),
            "single-node graph should run: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn run_linear_chain() {
        // START → a → b → c (terminates)
        let mut graph = GraphBuilder::new("START", EchoNode::new("a"))
            .node("a", EchoNode::new("b").with_suffix("-A"))
            .node("b", EchoNode::new("c").with_suffix("-B"))
            .node("c", TerminateNode)
            .edge("START", "a")
            .edge("a", "b")
            .edge("b", "c")
            .build()
            .unwrap();

        let result = graph.run().await.unwrap();

        // START (None) → "start"
        // a (Some("start")) → "start-A"
        // b (Some("start-A")) → "start-A-B"
        // c → Terminate, returns default
        assert_eq!(result.output, None, "terminate returns None output");
    }

    #[tokio::test]
    async fn run_entry_receives_none() {
        // Verifies the first node gets last = None.
        struct CheckNoneNode;

        impl INode for CheckNoneNode {
            fn step(&mut self, last: Option<NodeMonad>) -> anyhow::Result<Move> {
                assert!(last.is_none(), "entry node should receive None");
                Ok(Move::Terminate)
            }
        }

        let mut graph = GraphBuilder::new("START", CheckNoneNode).build().unwrap();
        graph.run().await.unwrap();
    }

    #[tokio::test]
    async fn run_invalid_transition() {
        // START tries to go to "c" but the only edge is START → "b".
        let mut graph = GraphBuilder::new("START", EchoNode::new("c"))
            .node("b", TerminateNode)
            .edge("START", "b")
            .build()
            .unwrap();

        let result = graph.run().await;
        assert!(result.is_err(), "invalid transition should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid transition"),
            "error should mention invalid transition, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn run_terminal_node_followed_by_next() {
        // Even if a node after terminate exists, terminate ends the graph.
        let mut graph = GraphBuilder::new("START", TerminateNode)
            .node("b", EchoNode::new("c"))
            .edge("START", "b")
            .build()
            .unwrap();

        graph.run().await.unwrap();
        // No assertion needed — if it panicked or looped, the test would hang.
    }

    #[tokio::test]
    async fn run_loop_terminates() {
        // START → looper → looper → ... → terminate after 5 steps
        let mut graph = GraphBuilder::new("START", EchoNode::new("looper").with_suffix("!"))
            .node(
                "looper",
                EchoNode::new("looper").with_suffix("?").terminate_after(4),
            )
            .edge("START", "looper")
            .edge("looper", "looper")
            .build()
            .unwrap();

        let result = graph.run().await;
        assert!(result.is_ok(), "loop should terminate: {:?}", result.err());
    }
}
