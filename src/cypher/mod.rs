use crate::core::GraphCore;
use crate::models::{EdgeRecord, NodeRecord, PropertyMap, PropertyValue};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type CypherParams = BTreeMap<String, CypherValue>;

#[derive(Clone, Debug)]
pub(crate) struct CypherResult {
    pub(crate) keys: Vec<String>,
    pub(crate) records: Vec<Vec<CypherValue>>,
    pub(crate) summary: CypherSummary,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CypherSummary {
    pub(crate) statement_type: String,
    pub(crate) nodes_created: usize,
    pub(crate) relationships_created: usize,
    pub(crate) properties_set: usize,
    pub(crate) properties_removed: usize,
    pub(crate) labels_added: usize,
    pub(crate) labels_removed: usize,
    pub(crate) nodes_deleted: usize,
    pub(crate) relationships_deleted: usize,
    pub(crate) rows: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum CypherValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Node(NodeRecord),
    Relationship(EdgeRecord),
    List(Vec<CypherValue>),
    Map(BTreeMap<String, CypherValue>),
}

#[derive(Clone, Debug)]
struct Query {
    statement: Statement,
}

#[derive(Clone, Debug)]
enum Statement {
    Return(ReturnQuery),
    Match(MatchQuery),
    Create(WriteQuery),
    Merge(WriteQuery),
    Union(Vec<Query>),
}

#[derive(Clone, Debug)]
struct ReturnQuery {
    projection: Projection,
}

#[derive(Clone, Debug)]
struct MatchQuery {
    optional: bool,
    pattern: Pattern,
    where_terms: Vec<Predicate>,
    action: MatchAction,
}

#[derive(Clone, Debug)]
enum MatchAction {
    Return(Projection),
    Update {
        set_items: Vec<SetItem>,
        remove_items: Vec<RemoveItem>,
        projection: Option<Projection>,
    },
    Delete {
        detach: bool,
        variables: Vec<String>,
    },
}

#[derive(Clone, Debug)]
enum SetItem {
    Property {
        var: String,
        key: String,
        value: Expr,
    },
    MergeMap {
        var: String,
        value: Expr,
    },
    Labels {
        var: String,
        labels: Vec<String>,
    },
}

#[derive(Clone, Debug)]
enum RemoveItem {
    Property { var: String, key: String },
    Labels { var: String, labels: Vec<String> },
}

#[derive(Clone, Debug)]
struct WriteQuery {
    pattern: Pattern,
    projection: Option<Projection>,
}

#[derive(Clone, Debug)]
struct Projection {
    items: Vec<ReturnItem>,
    order_by: Vec<OrderItem>,
    skip: Option<usize>,
    limit: Option<usize>,
}

#[derive(Clone, Debug)]
struct ReturnItem {
    expr: Expr,
    alias: String,
}

#[derive(Clone, Debug)]
struct OrderItem {
    expr: Expr,
    descending: bool,
}

#[derive(Clone, Debug)]
struct Pattern {
    nodes: Vec<NodePattern>,
    rels: Vec<RelPattern>,
}

#[derive(Clone, Debug)]
struct NodePattern {
    var: Option<String>,
    labels: Vec<String>,
    properties: Vec<(String, Expr)>,
}

#[derive(Clone, Debug)]
struct RelPattern {
    var: Option<String>,
    rel_type: Option<String>,
    direction: RelDirection,
    properties: Vec<(String, Expr)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelDirection {
    Out,
    In,
    Both,
}

#[derive(Clone, Debug)]
struct Predicate {
    left: Expr,
    op: PredicateOp,
    right: Expr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PredicateOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Contains,
    In,
}

#[derive(Clone, Debug)]
enum Expr {
    Literal(CypherValue),
    List(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Parameter(String),
    Var(String),
    Property(String, String),
    Id(String),
    ElementId(String),
    Labels(String),
    Type(String),
    StartNode(String),
    EndNode(String),
    CountAll,
    Count(String),
}

#[derive(Clone, Debug)]
enum Binding {
    Node(u64),
    Relationship(u64),
}

type BindingMap = BTreeMap<String, Binding>;

pub(crate) fn execute_autocommit(
    core: &mut GraphCore,
    query: &str,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    let query = parse_query(query)?;
    if query.is_write() {
        let (mut staged, base_version) = core.transaction_snapshot();
        let result = execute_query(&mut staged, &query, params, true)?;
        core.commit_transaction_snapshot(&staged, base_version)?;
        Ok(result)
    } else {
        execute_readonly(core, &query, params)
    }
}

pub(crate) fn execute_transaction(
    core: &mut GraphCore,
    query: &str,
    params: &CypherParams,
    write_allowed: bool,
) -> Result<CypherResult, String> {
    let query = parse_query(query)?;
    execute_query(core, &query, params, write_allowed)
}

pub(crate) fn execute_snapshot(
    core: &GraphCore,
    query: &str,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    let query = parse_query(query)?;
    if query.is_write() {
        return Err("GraphSnapshot.cypher() cannot execute write queries".to_string());
    }
    execute_readonly(core, &query, params)
}

impl Query {
    fn is_write(&self) -> bool {
        match &self.statement {
            Statement::Create(_) | Statement::Merge(_) => true,
            Statement::Match(query) => !matches!(query.action, MatchAction::Return(_)),
            Statement::Union(queries) => queries.iter().any(Query::is_write),
            Statement::Return(_) => false,
        }
    }
}

fn execute_readonly(
    core: &GraphCore,
    query: &Query,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    let mut scratch = core.snapshot();
    execute_query(&mut scratch, query, params, false)
}

fn execute_query(
    core: &mut GraphCore,
    query: &Query,
    params: &CypherParams,
    write_allowed: bool,
) -> Result<CypherResult, String> {
    match &query.statement {
        Statement::Return(query) => execute_return(core, query, params),
        Statement::Match(query) => {
            if !matches!(query.action, MatchAction::Return(_)) && !write_allowed {
                return Err("write query requires a writable Graph transaction".to_string());
            }
            execute_match(core, query, params)
        }
        Statement::Create(query) => {
            if !write_allowed {
                return Err("write query requires a writable Graph transaction".to_string());
            }
            execute_create(core, query, params)
        }
        Statement::Merge(query) => {
            if !write_allowed {
                return Err("write query requires a writable Graph transaction".to_string());
            }
            execute_merge(core, query, params)
        }
        Statement::Union(queries) => execute_union(core, queries, params, write_allowed),
    }
}

fn execute_union(
    core: &mut GraphCore,
    queries: &[Query],
    params: &CypherParams,
    write_allowed: bool,
) -> Result<CypherResult, String> {
    let mut keys = Vec::new();
    let mut records = Vec::new();
    for (index, query) in queries.iter().enumerate() {
        let result = execute_query(core, query, params, write_allowed)?;
        if index == 0 {
            keys = result.keys;
        } else if result.keys != keys {
            return Err("UNION queries must return the same columns".to_string());
        }
        records.extend(result.records);
    }
    let rows = records.len();
    Ok(CypherResult {
        keys,
        records,
        summary: CypherSummary {
            statement_type: "read".to_string(),
            rows,
            ..CypherSummary::default()
        },
    })
}

fn execute_return(
    core: &GraphCore,
    query: &ReturnQuery,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    project_rows(
        core,
        vec![BindingMap::new()],
        &query.projection,
        params,
        "read",
    )
}

fn execute_match(
    core: &mut GraphCore,
    query: &MatchQuery,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    validate_match_action(query)?;
    let mut rows = match_pattern(core, &query.pattern, params)?;
    if !query.where_terms.is_empty() {
        rows.retain(|binding| predicates_match(core, binding, &query.where_terms, params));
    }
    if query.optional && rows.is_empty() {
        rows.push(BindingMap::new());
    }
    match &query.action {
        MatchAction::Return(projection) => project_rows(core, rows, projection, params, "read"),
        MatchAction::Update {
            set_items,
            remove_items,
            projection,
        } => execute_update(core, rows, set_items, remove_items, projection, params),
        MatchAction::Delete { detach, variables } => execute_delete(core, rows, *detach, variables),
    }
}

fn validate_match_action(query: &MatchQuery) -> Result<(), String> {
    let mut aliases = BTreeMap::new();
    for node in &query.pattern.nodes {
        if let Some(var) = &node.var {
            aliases.insert(var.clone(), true);
        }
    }
    for rel in &query.pattern.rels {
        if let Some(var) = &rel.var {
            aliases.insert(var.clone(), false);
        }
    }
    let require = |var: &str| {
        aliases
            .get(var)
            .copied()
            .ok_or_else(|| format!("unknown Cypher variable {var:?}"))
    };
    match &query.action {
        MatchAction::Return(_) => Ok(()),
        MatchAction::Update {
            set_items,
            remove_items,
            ..
        } => {
            for item in set_items {
                match item {
                    SetItem::Property { var, .. } | SetItem::MergeMap { var, .. } => {
                        require(var)?;
                    }
                    SetItem::Labels { var, .. } => {
                        if !require(var)? {
                            return Err(format!("SET labels requires node variable {var:?}"));
                        }
                    }
                }
            }
            for item in remove_items {
                match item {
                    RemoveItem::Property { var, .. } => {
                        require(var)?;
                    }
                    RemoveItem::Labels { var, .. } => {
                        if !require(var)? {
                            return Err(format!("REMOVE labels requires node variable {var:?}"));
                        }
                    }
                }
            }
            Ok(())
        }
        MatchAction::Delete { variables, .. } => {
            for var in variables {
                require(var)?;
            }
            Ok(())
        }
    }
}

fn execute_update(
    core: &mut GraphCore,
    rows: Vec<BindingMap>,
    set_items: &[SetItem],
    remove_items: &[RemoveItem],
    projection: &Option<Projection>,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    let mut summary = CypherSummary {
        statement_type: "write".to_string(),
        ..CypherSummary::default()
    };
    for bindings in &rows {
        for item in set_items {
            apply_set_item(core, bindings, item, params, &mut summary)?;
        }
        for item in remove_items {
            apply_remove_item(core, bindings, item, &mut summary)?;
        }
    }
    match projection {
        Some(projection) => {
            let mut result = project_rows(core, rows, projection, params, "write")?;
            copy_write_summary(&summary, &mut result.summary);
            Ok(result)
        }
        None => Ok(CypherResult {
            keys: Vec::new(),
            records: Vec::new(),
            summary,
        }),
    }
}

fn apply_set_item(
    core: &mut GraphCore,
    bindings: &BindingMap,
    item: &SetItem,
    params: &CypherParams,
    summary: &mut CypherSummary,
) -> Result<(), String> {
    match item {
        SetItem::Property { var, key, value } => {
            let value = eval_expr(core, bindings, value, params)?;
            apply_property_value(core, bindings, var, key, value, summary)
        }
        SetItem::MergeMap { var, value } => {
            let value = eval_expr(core, bindings, value, params)?;
            let CypherValue::Map(values) = value else {
                return Err(format!("SET {var} += requires a map value"));
            };
            for (key, value) in values {
                apply_property_value(core, bindings, var, &key, value, summary)?;
            }
            Ok(())
        }
        SetItem::Labels { var, labels } => {
            let Some(Binding::Node(node_id)) = bindings.get(var) else {
                return Err(format!("SET labels requires node variable {var:?}"));
            };
            let before = core
                .get_node(*node_id)
                .ok_or_else(|| format!("node {node_id} not found"))?;
            let added = labels
                .iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|label| !before.labels.contains(label))
                .count();
            core.update_node_in_place(
                *node_id,
                None,
                labels.clone(),
                Vec::new(),
                PropertyMap::new(),
                Vec::new(),
            )?;
            summary.labels_added += added;
            Ok(())
        }
    }
}

fn apply_property_value(
    core: &mut GraphCore,
    bindings: &BindingMap,
    var: &str,
    key: &str,
    value: CypherValue,
    summary: &mut CypherSummary,
) -> Result<(), String> {
    let binding = bindings
        .get(var)
        .ok_or_else(|| format!("unknown Cypher variable {var:?}"))?
        .clone();
    match binding {
        Binding::Node(node_id) => {
            let before = core
                .get_node(node_id)
                .ok_or_else(|| format!("node {node_id} not found"))?;
            if key == "external_id" {
                let CypherValue::String(external_id) = value else {
                    return Err("external_id must be set to a non-null string".to_string());
                };
                if before.external_id != external_id {
                    core.update_node_in_place(
                        node_id,
                        Some(external_id),
                        Vec::new(),
                        Vec::new(),
                        PropertyMap::new(),
                        Vec::new(),
                    )?;
                    summary.properties_set += 1;
                }
                return Ok(());
            }
            match value {
                CypherValue::Null => {
                    if before.properties.contains_key(key) {
                        core.update_node_in_place(
                            node_id,
                            None,
                            Vec::new(),
                            Vec::new(),
                            PropertyMap::new(),
                            vec![key.to_string()],
                        )?;
                        summary.properties_removed += 1;
                    }
                }
                value => {
                    let property = value.as_property_value().ok_or_else(|| {
                        format!("property {key:?} must be a scalar Cypher property value")
                    })?;
                    if before.properties.get(key) != Some(&property) {
                        core.update_node_in_place(
                            node_id,
                            None,
                            Vec::new(),
                            Vec::new(),
                            PropertyMap::from([(key.to_string(), property)]),
                            Vec::new(),
                        )?;
                        summary.properties_set += 1;
                    }
                }
            }
        }
        Binding::Relationship(edge_id) => {
            if key == "external_id" {
                return Err("relationships do not have external_id".to_string());
            }
            let before = core
                .get_edge(edge_id)
                .ok_or_else(|| format!("edge {edge_id} not found"))?;
            match value {
                CypherValue::Null => {
                    if before.properties.contains_key(key) {
                        core.update_edge_in_place(
                            edge_id,
                            PropertyMap::new(),
                            vec![key.to_string()],
                        )?;
                        summary.properties_removed += 1;
                    }
                }
                value => {
                    let property = value.as_property_value().ok_or_else(|| {
                        format!("property {key:?} must be a scalar Cypher property value")
                    })?;
                    if before.properties.get(key) != Some(&property) {
                        core.update_edge_in_place(
                            edge_id,
                            PropertyMap::from([(key.to_string(), property)]),
                            Vec::new(),
                        )?;
                        summary.properties_set += 1;
                    }
                }
            }
        }
    }
    Ok(())
}

fn apply_remove_item(
    core: &mut GraphCore,
    bindings: &BindingMap,
    item: &RemoveItem,
    summary: &mut CypherSummary,
) -> Result<(), String> {
    match item {
        RemoveItem::Property { var, key } => {
            if key == "external_id" {
                return Err("external_id cannot be removed".to_string());
            }
            let binding = bindings
                .get(var)
                .ok_or_else(|| format!("unknown Cypher variable {var:?}"))?
                .clone();
            match binding {
                Binding::Node(node_id) => {
                    let before = core.get_node(node_id).unwrap();
                    if before.properties.contains_key(key) {
                        core.update_node_in_place(
                            node_id,
                            None,
                            Vec::new(),
                            Vec::new(),
                            PropertyMap::new(),
                            vec![key.clone()],
                        )?;
                        summary.properties_removed += 1;
                    }
                }
                Binding::Relationship(edge_id) => {
                    let before = core.get_edge(edge_id).unwrap();
                    if before.properties.contains_key(key) {
                        core.update_edge_in_place(edge_id, PropertyMap::new(), vec![key.clone()])?;
                        summary.properties_removed += 1;
                    }
                }
            }
            Ok(())
        }
        RemoveItem::Labels { var, labels } => {
            let Some(Binding::Node(node_id)) = bindings.get(var) else {
                return Err(format!("REMOVE labels requires node variable {var:?}"));
            };
            let before = core.get_node(*node_id).unwrap();
            let removed = labels
                .iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|label| before.labels.contains(label))
                .count();
            if removed > 0 {
                core.update_node_in_place(
                    *node_id,
                    None,
                    Vec::new(),
                    labels.clone(),
                    PropertyMap::new(),
                    Vec::new(),
                )?;
                summary.labels_removed += removed;
            }
            Ok(())
        }
    }
}

fn execute_delete(
    core: &mut GraphCore,
    rows: Vec<BindingMap>,
    detach: bool,
    variables: &[String],
) -> Result<CypherResult, String> {
    let before_nodes = core.node_count();
    let before_edges = core.edge_count();
    let mut node_ids = BTreeSet::new();
    let mut edge_ids = BTreeSet::new();
    for bindings in &rows {
        for var in variables {
            match bindings.get(var) {
                Some(Binding::Node(id)) => {
                    node_ids.insert(*id);
                }
                Some(Binding::Relationship(id)) => {
                    edge_ids.insert(*id);
                }
                None => return Err(format!("unknown Cypher variable {var:?}")),
            }
        }
    }
    for edge_id in edge_ids {
        if core.get_edge(edge_id).is_some() {
            core.delete_edge_in_place(edge_id)?;
        }
    }
    for node_id in node_ids {
        if core.get_node(node_id).is_some() {
            core.delete_node_in_place(node_id, detach)?;
        }
    }
    Ok(CypherResult {
        keys: Vec::new(),
        records: Vec::new(),
        summary: CypherSummary {
            statement_type: "write".to_string(),
            nodes_deleted: before_nodes - core.node_count(),
            relationships_deleted: before_edges - core.edge_count(),
            rows: 0,
            ..CypherSummary::default()
        },
    })
}

fn copy_write_summary(source: &CypherSummary, target: &mut CypherSummary) {
    target.nodes_created = source.nodes_created;
    target.relationships_created = source.relationships_created;
    target.properties_set = source.properties_set;
    target.properties_removed = source.properties_removed;
    target.labels_added = source.labels_added;
    target.labels_removed = source.labels_removed;
    target.nodes_deleted = source.nodes_deleted;
    target.relationships_deleted = source.relationships_deleted;
}

fn execute_create(
    core: &mut GraphCore,
    query: &WriteQuery,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    let mut summary = CypherSummary {
        statement_type: "write".to_string(),
        ..CypherSummary::default()
    };
    let binding = create_pattern(core, &query.pattern, params, &mut summary)?;
    project_write_result(core, binding, &query.projection, params, summary)
}

fn execute_merge(
    core: &mut GraphCore,
    query: &WriteQuery,
    params: &CypherParams,
) -> Result<CypherResult, String> {
    let matches = match_pattern(core, &query.pattern, params)?;
    if !matches.is_empty() {
        return project_write_result(
            core,
            matches[0].clone(),
            &query.projection,
            params,
            CypherSummary {
                statement_type: "write".to_string(),
                ..CypherSummary::default()
            },
        );
    }
    execute_create(core, query, params)
}

fn project_write_result(
    core: &GraphCore,
    binding: BindingMap,
    projection: &Option<Projection>,
    params: &CypherParams,
    mut summary: CypherSummary,
) -> Result<CypherResult, String> {
    match projection {
        Some(projection) => {
            let mut result = project_rows(core, vec![binding], projection, params, "write")?;
            copy_write_summary(&summary, &mut result.summary);
            Ok(result)
        }
        None => {
            summary.rows = 0;
            Ok(CypherResult {
                keys: Vec::new(),
                records: Vec::new(),
                summary,
            })
        }
    }
}

fn create_pattern(
    core: &mut GraphCore,
    pattern: &Pattern,
    params: &CypherParams,
    summary: &mut CypherSummary,
) -> Result<BindingMap, String> {
    let mut bindings = BindingMap::new();
    let mut node_ids = Vec::with_capacity(pattern.nodes.len());
    for node in &pattern.nodes {
        if let Some(var) = &node.var {
            if let Some(Binding::Node(id)) = bindings.get(var) {
                node_ids.push(*id);
                continue;
            }
        }
        let mut properties = eval_property_map(&node.properties, &bindings, core, params)?;
        let external_id = match properties.remove("external_id") {
            Some(PropertyValue::String(value)) => Some(value),
            Some(_) => return Err("external_id must be a string".to_string()),
            None => None,
        };
        let id = core.add_node(external_id, node.labels.clone(), properties)?;
        summary.nodes_created += 1;
        summary.properties_set += node.properties.len();
        if let Some(var) = &node.var {
            bindings.insert(var.clone(), Binding::Node(id));
        }
        node_ids.push(id);
    }

    for (index, rel) in pattern.rels.iter().enumerate() {
        if rel.direction == RelDirection::Both {
            return Err("CREATE requires directed relationship patterns".to_string());
        }
        if let Some(var) = &rel.var {
            if bindings.contains_key(var) {
                return Err(format!("relationship variable {var:?} is already bound"));
            }
        }
        let rel_type = rel
            .rel_type
            .clone()
            .ok_or_else(|| "CREATE relationships require a relationship type".to_string())?;
        let (source, target) = match rel.direction {
            RelDirection::Out => (node_ids[index], node_ids[index + 1]),
            RelDirection::In => (node_ids[index + 1], node_ids[index]),
            RelDirection::Both => unreachable!(),
        };
        let properties = eval_property_map(&rel.properties, &bindings, core, params)?;
        let id = core.add_edge(source, target, rel_type, properties)?;
        summary.relationships_created += 1;
        summary.properties_set += rel.properties.len();
        if let Some(var) = &rel.var {
            bindings.insert(var.clone(), Binding::Relationship(id));
        }
    }

    Ok(bindings)
}

fn match_pattern(
    core: &GraphCore,
    pattern: &Pattern,
    params: &CypherParams,
) -> Result<Vec<BindingMap>, String> {
    let mut rows = Vec::new();
    if pattern.nodes.is_empty() {
        return Ok(rows);
    }
    for node in core.nodes() {
        let mut bindings = BindingMap::new();
        if bind_node(&mut bindings, pattern.nodes[0].var.as_ref(), node.id)
            && node_matches(core, &node, &pattern.nodes[0], &bindings, params)?
        {
            expand_match(core, pattern, 0, node.id, bindings, params, &mut rows)?;
        }
    }
    Ok(rows)
}

fn expand_match(
    core: &GraphCore,
    pattern: &Pattern,
    node_index: usize,
    current_node: u64,
    bindings: BindingMap,
    params: &CypherParams,
    rows: &mut Vec<BindingMap>,
) -> Result<(), String> {
    if node_index >= pattern.rels.len() {
        rows.push(bindings);
        return Ok(());
    }

    let rel = &pattern.rels[node_index];
    for edge in core.edges() {
        let next_node = match rel.direction {
            RelDirection::Out if edge.source == current_node => Some(edge.target),
            RelDirection::In if edge.target == current_node => Some(edge.source),
            RelDirection::Both if edge.source == current_node => Some(edge.target),
            RelDirection::Both if edge.target == current_node => Some(edge.source),
            _ => None,
        };
        let Some(next_node) = next_node else {
            continue;
        };
        let Some(next_record) = core.get_node(next_node) else {
            continue;
        };
        let mut next_bindings = bindings.clone();
        if !bind_relationship(&mut next_bindings, rel.var.as_ref(), edge.id) {
            continue;
        }
        if !rel_matches(core, &edge, rel, &next_bindings, params)? {
            continue;
        }
        let next_pattern = &pattern.nodes[node_index + 1];
        if !bind_node(&mut next_bindings, next_pattern.var.as_ref(), next_node) {
            continue;
        }
        if !node_matches(core, &next_record, next_pattern, &next_bindings, params)? {
            continue;
        }
        expand_match(
            core,
            pattern,
            node_index + 1,
            next_node,
            next_bindings,
            params,
            rows,
        )?;
    }
    Ok(())
}

fn bind_node(bindings: &mut BindingMap, var: Option<&String>, id: u64) -> bool {
    bind(bindings, var, Binding::Node(id))
}

fn bind_relationship(bindings: &mut BindingMap, var: Option<&String>, id: u64) -> bool {
    bind(bindings, var, Binding::Relationship(id))
}

fn bind(bindings: &mut BindingMap, var: Option<&String>, binding: Binding) -> bool {
    let Some(var) = var else {
        return true;
    };
    match bindings.get(var) {
        Some(existing) => binding_eq(existing, &binding),
        None => {
            bindings.insert(var.clone(), binding);
            true
        }
    }
}

fn binding_eq(left: &Binding, right: &Binding) -> bool {
    matches!(
        (left, right),
        (Binding::Node(left), Binding::Node(right)) if left == right
    ) || matches!(
        (left, right),
        (Binding::Relationship(left), Binding::Relationship(right)) if left == right
    )
}

fn node_matches(
    core: &GraphCore,
    node: &NodeRecord,
    pattern: &NodePattern,
    bindings: &BindingMap,
    params: &CypherParams,
) -> Result<bool, String> {
    if pattern
        .labels
        .iter()
        .any(|label| !node.labels.contains(label))
    {
        return Ok(false);
    }
    for (key, expr) in &pattern.properties {
        let expected = eval_expr(core, bindings, expr, params)?;
        let actual = node_property_value(node, key);
        if !values_equal(&actual, &expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn rel_matches(
    core: &GraphCore,
    edge: &EdgeRecord,
    pattern: &RelPattern,
    bindings: &BindingMap,
    params: &CypherParams,
) -> Result<bool, String> {
    if pattern
        .rel_type
        .as_ref()
        .is_some_and(|rel_type| edge.edge_type != *rel_type)
    {
        return Ok(false);
    }
    for (key, expr) in &pattern.properties {
        let expected = eval_expr(core, bindings, expr, params)?;
        let actual = property_value(edge.properties.get(key));
        if !values_equal(&actual, &expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn predicates_match(
    core: &GraphCore,
    bindings: &BindingMap,
    predicates: &[Predicate],
    params: &CypherParams,
) -> bool {
    predicates
        .iter()
        .all(|predicate| predicate_matches(core, bindings, predicate, params).unwrap_or(false))
}

fn predicate_matches(
    core: &GraphCore,
    bindings: &BindingMap,
    predicate: &Predicate,
    params: &CypherParams,
) -> Result<bool, String> {
    let left = eval_expr(core, bindings, &predicate.left, params)?;
    let right = eval_expr(core, bindings, &predicate.right, params)?;
    Ok(match predicate.op {
        PredicateOp::Eq => values_equal(&left, &right),
        PredicateOp::Ne => !values_equal(&left, &right),
        PredicateOp::Lt => compare_values(&left, &right).is_some_and(|ord| ord == Ordering::Less),
        PredicateOp::Lte => {
            compare_values(&left, &right).is_some_and(|ord| ord != Ordering::Greater)
        }
        PredicateOp::Gt => {
            compare_values(&left, &right).is_some_and(|ord| ord == Ordering::Greater)
        }
        PredicateOp::Gte => compare_values(&left, &right).is_some_and(|ord| ord != Ordering::Less),
        PredicateOp::Contains => match (&left, &right) {
            (CypherValue::String(left), CypherValue::String(right)) => left.contains(right),
            _ => false,
        },
        PredicateOp::In => match right {
            CypherValue::List(values) => values.iter().any(|value| values_equal(&left, value)),
            _ => false,
        },
    })
}

fn project_rows(
    core: &GraphCore,
    bindings: Vec<BindingMap>,
    projection: &Projection,
    params: &CypherParams,
    statement_type: &str,
) -> Result<CypherResult, String> {
    let keys = projection
        .items
        .iter()
        .map(|item| item.alias.clone())
        .collect::<Vec<_>>();

    if projection
        .items
        .iter()
        .all(|item| matches!(item.expr, Expr::CountAll | Expr::Count(_)))
    {
        let record = projection
            .items
            .iter()
            .map(|item| match &item.expr {
                Expr::CountAll => CypherValue::Int(bindings.len() as i64),
                Expr::Count(var) => CypherValue::Int(
                    bindings
                        .iter()
                        .filter(|binding| binding.contains_key(var))
                        .count() as i64,
                ),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return Ok(CypherResult {
            keys,
            records: vec![record],
            summary: CypherSummary {
                statement_type: statement_type.to_string(),
                rows: 1,
                ..CypherSummary::default()
            },
        });
    }

    let mut sortable_rows = bindings
        .into_iter()
        .map(|binding| {
            let values = projection
                .items
                .iter()
                .map(|item| eval_expr(core, &binding, &item.expr, params))
                .collect::<Result<Vec<_>, _>>()?;
            let order_values = projection
                .order_by
                .iter()
                .map(|item| {
                    if let Expr::Var(alias) = &item.expr {
                        if let Some(index) = projection
                            .items
                            .iter()
                            .position(|return_item| return_item.alias == *alias)
                        {
                            return Ok(values[index].clone());
                        }
                    }
                    eval_expr(core, &binding, &item.expr, params)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((values, order_values))
        })
        .collect::<Result<Vec<_>, String>>()?;

    if !projection.order_by.is_empty() {
        sortable_rows.sort_by(|left, right| {
            for (index, item) in projection.order_by.iter().enumerate() {
                let ordering = compare_for_sort(&left.1[index], &right.1[index]);
                if ordering != Ordering::Equal {
                    return if item.descending {
                        ordering.reverse()
                    } else {
                        ordering
                    };
                }
            }
            Ordering::Equal
        });
    }

    let skip = projection.skip.unwrap_or(0);
    let iter = sortable_rows.into_iter().skip(skip);
    let records: Vec<Vec<CypherValue>> = match projection.limit {
        Some(limit) => iter.take(limit).map(|(values, _)| values).collect(),
        None => iter.map(|(values, _)| values).collect(),
    };
    let rows = records.len();
    Ok(CypherResult {
        keys,
        records,
        summary: CypherSummary {
            statement_type: statement_type.to_string(),
            rows,
            ..CypherSummary::default()
        },
    })
}

fn eval_property_map(
    properties: &[(String, Expr)],
    bindings: &BindingMap,
    core: &GraphCore,
    params: &CypherParams,
) -> Result<PropertyMap, String> {
    let mut result = PropertyMap::new();
    for (key, expr) in properties {
        let value = eval_expr(core, bindings, expr, params)?;
        let value = value
            .as_property_value()
            .ok_or_else(|| format!("property {key:?} must be a scalar Cypher property value"))?;
        result.insert(key.clone(), value);
    }
    Ok(result)
}

fn eval_expr(
    core: &GraphCore,
    bindings: &BindingMap,
    expr: &Expr,
    params: &CypherParams,
) -> Result<CypherValue, String> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::List(values) => Ok(CypherValue::List(
            values
                .iter()
                .map(|value| eval_expr(core, bindings, value, params))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::Map(values) => Ok(CypherValue::Map(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), eval_expr(core, bindings, value, params)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()?,
        )),
        Expr::Parameter(name) => params
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing Cypher parameter ${name}")),
        Expr::Var(var) => match bindings.get(var) {
            Some(Binding::Node(id)) => Ok(core
                .get_node(*id)
                .map(CypherValue::Node)
                .unwrap_or(CypherValue::Null)),
            Some(Binding::Relationship(id)) => Ok(core
                .get_edge(*id)
                .map(CypherValue::Relationship)
                .unwrap_or(CypherValue::Null)),
            None => Ok(CypherValue::Null),
        },
        Expr::Property(var, key) => match bindings.get(var) {
            Some(Binding::Node(id)) => Ok(core
                .get_node(*id)
                .as_ref()
                .map(|node| node_property_value(node, key))
                .unwrap_or(CypherValue::Null)),
            Some(Binding::Relationship(id)) => Ok(core
                .get_edge(*id)
                .as_ref()
                .map(|edge| property_value(edge.properties.get(key)))
                .unwrap_or(CypherValue::Null)),
            None => Ok(CypherValue::Null),
        },
        Expr::Id(var) => match bindings.get(var) {
            Some(Binding::Node(id)) | Some(Binding::Relationship(id)) => {
                Ok(CypherValue::Int(*id as i64))
            }
            None => Ok(CypherValue::Null),
        },
        Expr::ElementId(var) => match bindings.get(var) {
            Some(Binding::Node(id)) => Ok(CypherValue::String(format!("node:{id}"))),
            Some(Binding::Relationship(id)) => {
                Ok(CypherValue::String(format!("relationship:{id}")))
            }
            None => Ok(CypherValue::Null),
        },
        Expr::Labels(var) => match bindings.get(var) {
            Some(Binding::Node(id)) => Ok(core
                .get_node(*id)
                .map(|node| {
                    CypherValue::List(node.labels.into_iter().map(CypherValue::String).collect())
                })
                .unwrap_or(CypherValue::Null)),
            _ => Ok(CypherValue::Null),
        },
        Expr::Type(var) => match bindings.get(var) {
            Some(Binding::Relationship(id)) => Ok(core
                .get_edge(*id)
                .map(|edge| CypherValue::String(edge.edge_type))
                .unwrap_or(CypherValue::Null)),
            _ => Ok(CypherValue::Null),
        },
        Expr::StartNode(var) => match bindings.get(var) {
            Some(Binding::Relationship(id)) => Ok(core
                .get_edge(*id)
                .and_then(|edge| core.get_node(edge.source))
                .map(CypherValue::Node)
                .unwrap_or(CypherValue::Null)),
            _ => Ok(CypherValue::Null),
        },
        Expr::EndNode(var) => match bindings.get(var) {
            Some(Binding::Relationship(id)) => Ok(core
                .get_edge(*id)
                .and_then(|edge| core.get_node(edge.target))
                .map(CypherValue::Node)
                .unwrap_or(CypherValue::Null)),
            _ => Ok(CypherValue::Null),
        },
        Expr::CountAll | Expr::Count(_) => {
            Err("aggregate expression is only valid in RETURN".to_string())
        }
    }
}

impl CypherValue {
    fn as_property_value(&self) -> Option<PropertyValue> {
        match self {
            CypherValue::Bool(value) => Some(PropertyValue::Bool(*value)),
            CypherValue::Int(value) => Some(PropertyValue::Int(*value)),
            CypherValue::Float(value) if value.is_finite() => Some(PropertyValue::Float(*value)),
            CypherValue::String(value) => Some(PropertyValue::String(value.clone())),
            CypherValue::Null
            | CypherValue::Float(_)
            | CypherValue::Node(_)
            | CypherValue::Relationship(_)
            | CypherValue::List(_)
            | CypherValue::Map(_) => None,
        }
    }
}

fn node_property_value(node: &NodeRecord, key: &str) -> CypherValue {
    node.properties
        .get(key)
        .map(property_value_from_ref)
        .unwrap_or_else(|| {
            if key == "external_id" {
                CypherValue::String(node.external_id.clone())
            } else {
                CypherValue::Null
            }
        })
}

fn property_value(value: Option<&PropertyValue>) -> CypherValue {
    value
        .map(property_value_from_ref)
        .unwrap_or(CypherValue::Null)
}

fn property_value_from_ref(value: &PropertyValue) -> CypherValue {
    match value {
        PropertyValue::Bool(value) => CypherValue::Bool(*value),
        PropertyValue::Int(value) => CypherValue::Int(*value),
        PropertyValue::Float(value) => CypherValue::Float(*value),
        PropertyValue::String(value) => CypherValue::String(value.clone()),
    }
}

fn values_equal(left: &CypherValue, right: &CypherValue) -> bool {
    match (left, right) {
        (CypherValue::Null, _) | (_, CypherValue::Null) => false,
        (CypherValue::Bool(left), CypherValue::Bool(right)) => left == right,
        (CypherValue::Int(left), CypherValue::Int(right)) => left == right,
        (CypherValue::Float(left), CypherValue::Float(right)) => left == right,
        (CypherValue::Int(left), CypherValue::Float(right)) => (*left as f64) == *right,
        (CypherValue::Float(left), CypherValue::Int(right)) => *left == (*right as f64),
        (CypherValue::String(left), CypherValue::String(right)) => left == right,
        (CypherValue::Node(left), CypherValue::Node(right)) => left.id == right.id,
        (CypherValue::Relationship(left), CypherValue::Relationship(right)) => left.id == right.id,
        _ => false,
    }
}

fn compare_values(left: &CypherValue, right: &CypherValue) -> Option<Ordering> {
    match (left, right) {
        (CypherValue::Int(left), CypherValue::Int(right)) => Some(left.cmp(right)),
        (CypherValue::Float(left), CypherValue::Float(right)) => left.partial_cmp(right),
        (CypherValue::Int(left), CypherValue::Float(right)) => (*left as f64).partial_cmp(right),
        (CypherValue::Float(left), CypherValue::Int(right)) => left.partial_cmp(&(*right as f64)),
        (CypherValue::String(left), CypherValue::String(right)) => Some(left.cmp(right)),
        _ => None,
    }
}

fn compare_for_sort(left: &CypherValue, right: &CypherValue) -> Ordering {
    match (left, right) {
        (CypherValue::Null, CypherValue::Null) => Ordering::Equal,
        (CypherValue::Null, _) => Ordering::Greater,
        (_, CypherValue::Null) => Ordering::Less,
        _ => compare_values(left, right).unwrap_or(Ordering::Equal),
    }
}

fn parse_query(query: &str) -> Result<Query, String> {
    let query = query.trim().trim_end_matches(';').trim();
    if query.is_empty() {
        return Err("Cypher query cannot be empty".to_string());
    }
    let union_parts = split_keyword_top_level(query, "UNION");
    if union_parts.len() > 1 {
        let queries = union_parts
            .into_iter()
            .map(parse_query)
            .collect::<Result<Vec<_>, _>>()?;
        if queries.iter().any(Query::is_write) {
            return Err("UNION only supports read queries".to_string());
        }
        return Ok(Query {
            statement: Statement::Union(queries),
        });
    }

    let upper = query.to_ascii_uppercase();
    if upper.starts_with("OPTIONAL MATCH ") {
        parse_match_query(query, true)
    } else if upper.starts_with("MATCH ") {
        parse_match_query(query, false)
    } else if upper.starts_with("CREATE ") {
        parse_write_query(query, "CREATE", false)
    } else if upper.starts_with("MERGE ") {
        parse_write_query(query, "MERGE", true)
    } else if upper.starts_with("RETURN ") {
        Ok(Query {
            statement: Statement::Return(ReturnQuery {
                projection: parse_projection(&query["RETURN".len()..])?,
            }),
        })
    } else {
        Err("unsupported Cypher statement; supported clauses are MATCH, OPTIONAL MATCH, RETURN, CREATE, MERGE, and UNION".to_string())
    }
}

fn parse_match_query(query: &str, optional: bool) -> Result<Query, String> {
    let prefix = if optional { "OPTIONAL MATCH" } else { "MATCH" };
    let input = query[prefix.len()..].trim();
    let clauses = [
        "WHERE",
        "SET",
        "REMOVE",
        "DETACH DELETE",
        "DELETE",
        "RETURN",
    ];
    let first_clause = next_clause_index(input, 0, &clauses)
        .ok_or_else(|| "MATCH query requires RETURN or a write clause".to_string())?;
    let pattern = parse_pattern(input[..first_clause].trim())?;
    let mut cursor = first_clause;
    let mut where_terms = Vec::new();

    let tail = input[cursor..].trim_start();
    cursor = input.len() - tail.len();
    if keyword_at(tail, "WHERE") {
        let start = cursor + "WHERE".len();
        let end = next_clause_index(
            input,
            start,
            &["SET", "REMOVE", "DETACH DELETE", "DELETE", "RETURN"],
        )
        .unwrap_or(input.len());
        where_terms = parse_predicates(input[start..end].trim())?;
        cursor = end;
    }

    let mut set_items = Vec::new();
    let mut remove_items = Vec::new();
    let mut projection = None;
    let mut seen_remove = false;
    while cursor < input.len() {
        let tail = input[cursor..].trim_start();
        cursor = input.len() - tail.len();
        if keyword_at(tail, "SET") {
            if seen_remove {
                return Err("SET cannot follow REMOVE in the current Cypher subset".to_string());
            }
            let start = cursor + "SET".len();
            let end = next_clause_index(
                input,
                start,
                &["SET", "REMOVE", "DETACH DELETE", "DELETE", "RETURN"],
            )
            .unwrap_or(input.len());
            set_items.extend(parse_set_items(input[start..end].trim())?);
            cursor = end;
        } else if keyword_at(tail, "REMOVE") {
            seen_remove = true;
            let start = cursor + "REMOVE".len();
            let end = next_clause_index(
                input,
                start,
                &["SET", "REMOVE", "DETACH DELETE", "DELETE", "RETURN"],
            )
            .unwrap_or(input.len());
            remove_items.extend(parse_remove_items(input[start..end].trim())?);
            cursor = end;
        } else if keyword_at(tail, "DETACH DELETE") || keyword_at(tail, "DELETE") {
            if optional {
                return Err("OPTIONAL MATCH cannot be used with DELETE".to_string());
            }
            if !set_items.is_empty() || !remove_items.is_empty() {
                return Err("DELETE cannot be combined with SET or REMOVE".to_string());
            }
            let detach = keyword_at(tail, "DETACH DELETE");
            let keyword = if detach { "DETACH DELETE" } else { "DELETE" };
            let variables_text = input[cursor + keyword.len()..].trim();
            if find_keyword_top_level(variables_text, "RETURN").is_some() {
                return Err("DELETE ... RETURN is not supported".to_string());
            }
            let variables = parse_delete_variables(variables_text)?;
            return Ok(Query {
                statement: Statement::Match(MatchQuery {
                    optional,
                    pattern,
                    where_terms,
                    action: MatchAction::Delete { detach, variables },
                }),
            });
        } else if keyword_at(tail, "RETURN") {
            projection = Some(parse_projection(input[cursor + "RETURN".len()..].trim())?);
            cursor = input.len();
        } else {
            return Err(format!("unsupported MATCH clause near {tail:?}"));
        }
    }

    let action = if set_items.is_empty() && remove_items.is_empty() {
        MatchAction::Return(
            projection.ok_or_else(|| "read-only MATCH query requires RETURN".to_string())?,
        )
    } else {
        if optional {
            return Err("OPTIONAL MATCH cannot be used with SET or REMOVE".to_string());
        }
        MatchAction::Update {
            set_items,
            remove_items,
            projection,
        }
    };
    Ok(Query {
        statement: Statement::Match(MatchQuery {
            optional,
            pattern,
            where_terms,
            action,
        }),
    })
}

fn parse_set_items(input: &str) -> Result<Vec<SetItem>, String> {
    if input.is_empty() {
        return Err("SET requires at least one item".to_string());
    }
    split_top_level(input, ',')
        .into_iter()
        .map(parse_set_item)
        .collect()
}

fn parse_set_item(input: &str) -> Result<SetItem, String> {
    if let Some(index) = find_operator_top_level(input, "+=") {
        return Ok(SetItem::MergeMap {
            var: parse_identifier(input[..index].trim())?,
            value: parse_expr(input[index + 2..].trim())?,
        });
    }
    if let Some(index) = find_operator_top_level(input, "=") {
        let (var, key) = parse_property_access(input[..index].trim())?;
        return Ok(SetItem::Property {
            var,
            key,
            value: parse_expr(input[index + 1..].trim())?,
        });
    }
    let (var, labels) = parse_label_target(input)?;
    Ok(SetItem::Labels { var, labels })
}

fn parse_remove_items(input: &str) -> Result<Vec<RemoveItem>, String> {
    if input.is_empty() {
        return Err("REMOVE requires at least one item".to_string());
    }
    split_top_level(input, ',')
        .into_iter()
        .map(|item| {
            if item.contains('.') {
                let (var, key) = parse_property_access(item)?;
                Ok(RemoveItem::Property { var, key })
            } else {
                let (var, labels) = parse_label_target(item)?;
                Ok(RemoveItem::Labels { var, labels })
            }
        })
        .collect()
}

fn parse_property_access(input: &str) -> Result<(String, String), String> {
    let Some((var, key)) = input.split_once('.') else {
        return Err(format!("expected property access, got {input:?}"));
    };
    if key.contains('.') {
        return Err(format!("invalid property access {input:?}"));
    }
    Ok((parse_identifier(var)?, parse_property_key(key)?))
}

fn parse_label_target(input: &str) -> Result<(String, Vec<String>), String> {
    let mut parts = input.split(':');
    let var = parse_identifier(parts.next().unwrap_or_default())?;
    let labels = parts.map(parse_identifier).collect::<Result<Vec<_>, _>>()?;
    if labels.is_empty() {
        return Err(format!(
            "label update requires at least one label in {input:?}"
        ));
    }
    Ok((var, labels))
}

fn parse_delete_variables(input: &str) -> Result<Vec<String>, String> {
    let variables = split_top_level(input, ',')
        .into_iter()
        .map(parse_identifier)
        .collect::<Result<Vec<_>, _>>()?;
    if variables.is_empty() {
        return Err("DELETE requires at least one variable".to_string());
    }
    Ok(variables)
}

fn parse_write_query(query: &str, keyword: &str, merge: bool) -> Result<Query, String> {
    let after_prefix = query[keyword.len()..].trim();
    let (pattern_text, projection) = match find_keyword_top_level(after_prefix, "RETURN") {
        Some(return_index) => (
            after_prefix[..return_index].trim(),
            Some(parse_projection(
                after_prefix[return_index + "RETURN".len()..].trim(),
            )?),
        ),
        None => (after_prefix, None),
    };
    let write = WriteQuery {
        pattern: parse_pattern(pattern_text)?,
        projection,
    };
    Ok(Query {
        statement: if merge {
            Statement::Merge(write)
        } else {
            Statement::Create(write)
        },
    })
}

fn parse_projection(input: &str) -> Result<Projection, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("RETURN requires at least one expression".to_string());
    }

    let order_index = find_keyword_top_level(input, "ORDER BY");
    let skip_index = find_keyword_top_level(input, "SKIP");
    let limit_index = find_keyword_top_level(input, "LIMIT");
    let tail_start = [order_index, skip_index, limit_index]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(input.len());

    let item_text = input[..tail_start].trim();
    let mut order_by = Vec::new();
    let mut skip = None;
    let mut limit = None;
    let mut cursor = tail_start;
    while cursor < input.len() {
        let tail = input[cursor..].trim_start();
        cursor = input.len() - tail.len();
        if keyword_at(tail, "ORDER BY") {
            let start = cursor + "ORDER BY".len();
            let end = next_clause_index(input, start, &["SKIP", "LIMIT"]).unwrap_or(input.len());
            order_by = parse_order_items(input[start..end].trim())?;
            cursor = end;
        } else if keyword_at(tail, "SKIP") {
            let start = cursor + "SKIP".len();
            let end =
                next_clause_index(input, start, &["ORDER BY", "LIMIT"]).unwrap_or(input.len());
            skip = Some(parse_usize(input[start..end].trim(), "SKIP")?);
            cursor = end;
        } else if keyword_at(tail, "LIMIT") {
            let start = cursor + "LIMIT".len();
            let end = next_clause_index(input, start, &["ORDER BY", "SKIP"]).unwrap_or(input.len());
            limit = Some(parse_usize(input[start..end].trim(), "LIMIT")?);
            cursor = end;
        } else {
            return Err(format!("unsupported RETURN tail near {tail:?}"));
        }
    }

    Ok(Projection {
        items: split_top_level(item_text, ',')
            .into_iter()
            .map(parse_return_item)
            .collect::<Result<Vec<_>, _>>()?,
        order_by,
        skip,
        limit,
    })
}

fn parse_return_item(input: &str) -> Result<ReturnItem, String> {
    let input = input.trim();
    let (expr_text, alias) = match find_keyword_top_level(input, "AS") {
        Some(index) => (
            input[..index].trim(),
            parse_identifier(input[index + "AS".len()..].trim())?,
        ),
        None => (input, default_alias(input)),
    };
    Ok(ReturnItem {
        expr: parse_expr(expr_text)?,
        alias,
    })
}

fn parse_order_items(input: &str) -> Result<Vec<OrderItem>, String> {
    split_top_level(input, ',')
        .into_iter()
        .map(|item| {
            let mut text = item.trim();
            let mut descending = false;
            if ends_with_keyword(text, "DESC") || ends_with_keyword(text, "DESCENDING") {
                descending = true;
                text = trim_last_word(text);
            } else if ends_with_keyword(text, "ASC") || ends_with_keyword(text, "ASCENDING") {
                text = trim_last_word(text);
            }
            Ok(OrderItem {
                expr: parse_expr(text)?,
                descending,
            })
        })
        .collect()
}

fn parse_predicates(input: &str) -> Result<Vec<Predicate>, String> {
    split_keyword_top_level(input, "AND")
        .into_iter()
        .map(|term| parse_predicate(term.trim()))
        .collect()
}

fn parse_predicate(input: &str) -> Result<Predicate, String> {
    for (keyword, op) in [
        ("CONTAINS", PredicateOp::Contains),
        ("IN", PredicateOp::In),
        (">=", PredicateOp::Gte),
        ("<=", PredicateOp::Lte),
        ("<>", PredicateOp::Ne),
        ("!=", PredicateOp::Ne),
        ("=", PredicateOp::Eq),
        (">", PredicateOp::Gt),
        ("<", PredicateOp::Lt),
    ] {
        if let Some(index) = find_operator_top_level(input, keyword) {
            return Ok(Predicate {
                left: parse_expr(input[..index].trim())?,
                op,
                right: parse_expr(input[index + keyword.len()..].trim())?,
            });
        }
    }
    Err(format!("unsupported WHERE predicate {input:?}"))
}

fn parse_pattern(input: &str) -> Result<Pattern, String> {
    let input = input.trim();
    let mut cursor = 0;
    let mut nodes = Vec::new();
    let mut rels = Vec::new();
    nodes.push(parse_node_at(input, &mut cursor)?);
    loop {
        skip_ws(input, &mut cursor);
        if cursor >= input.len() {
            break;
        }
        rels.push(parse_rel_at(input, &mut cursor)?);
        nodes.push(parse_node_at(input, &mut cursor)?);
    }
    Ok(Pattern { nodes, rels })
}

fn parse_node_at(input: &str, cursor: &mut usize) -> Result<NodePattern, String> {
    skip_ws(input, cursor);
    if !input[*cursor..].starts_with('(') {
        return Err(format!(
            "expected node pattern near {:?}",
            &input[*cursor..]
        ));
    }
    let end = matching_delimiter(input, *cursor, '(', ')')?;
    let content = input[*cursor + 1..end].trim();
    *cursor = end + 1;
    parse_node_content(content)
}

fn parse_rel_at(input: &str, cursor: &mut usize) -> Result<RelPattern, String> {
    skip_ws(input, cursor);
    let prefix_incoming = if input[*cursor..].starts_with("<-") {
        *cursor += 2;
        true
    } else if input[*cursor..].starts_with('-') {
        *cursor += 1;
        false
    } else {
        return Err(format!(
            "expected relationship pattern near {:?}",
            &input[*cursor..]
        ));
    };
    skip_ws(input, cursor);
    if !input[*cursor..].starts_with('[') {
        return Err(format!("expected '[' near {:?}", &input[*cursor..]));
    }
    let end = matching_delimiter(input, *cursor, '[', ']')?;
    let content = input[*cursor + 1..end].trim();
    *cursor = end + 1;
    skip_ws(input, cursor);
    let outgoing = if input[*cursor..].starts_with("->") {
        *cursor += 2;
        true
    } else if input[*cursor..].starts_with('-') {
        *cursor += 1;
        false
    } else {
        return Err(format!(
            "relationship pattern must end with '-' or '->' near {:?}",
            &input[*cursor..]
        ));
    };
    let direction = match (prefix_incoming, outgoing) {
        (true, false) => RelDirection::In,
        (false, true) => RelDirection::Out,
        (false, false) => RelDirection::Both,
        (true, true) => {
            return Err("relationship pattern cannot use both '<-' and '->'".to_string())
        }
    };
    parse_rel_content(content, direction)
}

fn parse_node_content(content: &str) -> Result<NodePattern, String> {
    let (head, properties) = split_pattern_properties(content)?;
    let mut var = None;
    let mut labels = Vec::new();
    let mut cursor = 0;
    let head = head.trim();
    if !head.is_empty() && !head.starts_with(':') {
        let end = head.find(':').unwrap_or(head.len());
        var = Some(parse_identifier(head[..end].trim())?);
        cursor = end;
    }
    while cursor < head.len() {
        let rest = head[cursor..].trim_start();
        cursor = head.len() - rest.len();
        if !rest.starts_with(':') {
            return Err(format!("unsupported node pattern content {rest:?}"));
        }
        cursor += 1;
        let label_start = cursor;
        while cursor < head.len() && is_identifier_char(head.as_bytes()[cursor] as char) {
            cursor += 1;
        }
        labels.push(parse_identifier(&head[label_start..cursor])?);
    }
    Ok(NodePattern {
        var,
        labels,
        properties,
    })
}

fn parse_rel_content(content: &str, direction: RelDirection) -> Result<RelPattern, String> {
    let (head, properties) = split_pattern_properties(content)?;
    let head = head.trim();
    let mut var = None;
    let mut rel_type = None;
    if !head.is_empty() {
        if let Some(index) = head.find(':') {
            if index > 0 {
                var = Some(parse_identifier(head[..index].trim())?);
            }
            let rel_type_text = head[index + 1..].trim();
            if rel_type_text.contains('|') {
                return Err("multiple relationship types are not yet supported".to_string());
            }
            if !rel_type_text.is_empty() {
                rel_type = Some(parse_identifier(rel_type_text)?);
            }
        } else {
            var = Some(parse_identifier(head)?);
        }
    }
    Ok(RelPattern {
        var,
        rel_type,
        direction,
        properties,
    })
}

fn split_pattern_properties(content: &str) -> Result<(&str, Vec<(String, Expr)>), String> {
    if let Some(start) = find_char_top_level(content, '{') {
        let end = matching_delimiter(content, start, '{', '}')?;
        if !content[end + 1..].trim().is_empty() {
            return Err("unsupported pattern content after property map".to_string());
        }
        Ok((
            content[..start].trim(),
            parse_property_map(&content[start + 1..end])?,
        ))
    } else {
        Ok((content.trim(), Vec::new()))
    }
}

fn parse_property_map(input: &str) -> Result<Vec<(String, Expr)>, String> {
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    split_top_level(input, ',')
        .into_iter()
        .map(|item| {
            let colon = find_char_top_level(item, ':')
                .ok_or_else(|| format!("property map entry requires ':' in {item:?}"))?;
            let key = parse_property_key(item[..colon].trim())?;
            let value = parse_expr(item[colon + 1..].trim())?;
            Ok((key, value))
        })
        .collect()
}

fn parse_expr(input: &str) -> Result<Expr, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("empty expression".to_string());
    }
    if input == "*" {
        return Ok(Expr::CountAll);
    }
    if input.eq_ignore_ascii_case("count(*)") {
        return Ok(Expr::CountAll);
    }
    for (name, constructor) in [
        ("id", Expr::Id as fn(String) -> Expr),
        ("elementId", Expr::ElementId),
        ("labels", Expr::Labels),
        ("type", Expr::Type),
        ("startNode", Expr::StartNode),
        ("endNode", Expr::EndNode),
        ("count", Expr::Count),
    ] {
        if let Some(arg) = parse_function_arg(input, name) {
            return Ok(constructor(parse_identifier(arg.trim())?));
        }
    }
    if let Some(name) = input.strip_prefix('$') {
        return Ok(Expr::Parameter(parse_identifier(name)?));
    }
    if is_quoted(input) {
        return Ok(Expr::Literal(CypherValue::String(unquote(input)?)));
    }
    if input.eq_ignore_ascii_case("true") {
        return Ok(Expr::Literal(CypherValue::Bool(true)));
    }
    if input.eq_ignore_ascii_case("false") {
        return Ok(Expr::Literal(CypherValue::Bool(false)));
    }
    if input.eq_ignore_ascii_case("null") {
        return Ok(Expr::Literal(CypherValue::Null));
    }
    if input.starts_with('{') && input.ends_with('}') {
        return Ok(Expr::Map(parse_property_map(&input[1..input.len() - 1])?));
    }
    if input.starts_with('[') && input.ends_with(']') {
        return Ok(Expr::List(
            split_top_level(&input[1..input.len() - 1], ',')
                .into_iter()
                .filter(|item| !item.trim().is_empty())
                .map(|item| parse_expr(item.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Ok(value) = input.parse::<i64>() {
        return Ok(Expr::Literal(CypherValue::Int(value)));
    }
    if input.contains('.') {
        if let Ok(value) = input.parse::<f64>() {
            return Ok(Expr::Literal(CypherValue::Float(value)));
        }
        let parts = input.split('.').collect::<Vec<_>>();
        if parts.len() == 2 {
            return Ok(Expr::Property(
                parse_identifier(parts[0].trim())?,
                parse_property_key(parts[1].trim())?,
            ));
        }
    }
    Ok(Expr::Var(parse_identifier(input)?))
}

fn parse_function_arg<'a>(input: &'a str, name: &str) -> Option<&'a str> {
    if input.len() <= name.len() + 2 || !input[..name.len()].eq_ignore_ascii_case(name) {
        return None;
    }
    let rest = input[name.len()..].trim_start();
    if !rest.starts_with('(') || !rest.ends_with(')') {
        return None;
    }
    Some(&rest[1..rest.len() - 1])
}

fn parse_usize(input: &str, clause: &str) -> Result<usize, String> {
    input
        .parse()
        .map_err(|_| format!("{clause} requires a non-negative integer literal"))
}

fn default_alias(input: &str) -> String {
    input.trim().to_string()
}

fn parse_identifier(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.len() >= 2 && input.starts_with('`') && input.ends_with('`') {
        return Ok(input[1..input.len() - 1].to_string());
    }
    if input.is_empty() {
        return Err("identifier cannot be empty".to_string());
    }
    let mut chars = input.chars();
    let first = chars.next().unwrap();
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(format!("invalid identifier {input:?}"));
    }
    if chars.any(|ch| !is_identifier_char(ch)) {
        return Err(format!("invalid identifier {input:?}"));
    }
    Ok(input.to_string())
}

fn parse_property_key(input: &str) -> Result<String, String> {
    if is_quoted(input) {
        unquote(input)
    } else {
        parse_identifier(input)
    }
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_quoted(input: &str) -> bool {
    input.len() >= 2
        && ((input.starts_with('\'') && input.ends_with('\''))
            || (input.starts_with('"') && input.ends_with('"')))
}

fn unquote(input: &str) -> Result<String, String> {
    if !is_quoted(input) {
        return Err(format!("expected quoted string, got {input:?}"));
    }
    let mut result = String::new();
    let mut escaped = false;
    for ch in input[1..input.len() - 1].chars() {
        if escaped {
            result.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            result.push(ch);
        }
    }
    if escaped {
        return Err("unterminated string escape".to_string());
    }
    Ok(result)
}

fn split_keyword_top_level<'a>(input: &'a str, keyword: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    while let Some(index) = find_keyword_top_level_from(input, keyword, cursor) {
        parts.push(input[start..index].trim());
        cursor = index + keyword.len();
        start = cursor;
    }
    parts.push(input[start..].trim());
    parts
}

fn find_keyword_top_level(input: &str, keyword: &str) -> Option<usize> {
    find_keyword_top_level_from(input, keyword, 0)
}

fn find_keyword_top_level_from(input: &str, keyword: &str, start: usize) -> Option<usize> {
    let upper = input.to_ascii_uppercase();
    let keyword = keyword.to_ascii_uppercase();
    let mut state = ScanState::default();
    let bytes = input.as_bytes();
    let mut index = start;
    while index + keyword.len() <= input.len() {
        state.update(bytes[index] as char);
        if state.top_level()
            && upper[index..].starts_with(&keyword)
            && boundary(input, index, keyword.len())
        {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn keyword_at(input: &str, keyword: &str) -> bool {
    input
        .get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
        && boundary(input, 0, keyword.len())
}

fn ends_with_keyword(input: &str, keyword: &str) -> bool {
    let input = input.trim_end();
    input.len().checked_sub(keyword.len()).is_some_and(|start| {
        input[start..].eq_ignore_ascii_case(keyword) && boundary(input, start, keyword.len())
    })
}

fn trim_last_word(input: &str) -> &str {
    input
        .trim_end()
        .rsplit_once(char::is_whitespace)
        .map(|(head, _)| head.trim_end())
        .unwrap_or("")
}

fn next_clause_index(input: &str, start: usize, clauses: &[&str]) -> Option<usize> {
    clauses
        .iter()
        .filter_map(|clause| find_keyword_top_level_from(input, clause, start))
        .min()
}

fn find_operator_top_level(input: &str, operator: &str) -> Option<usize> {
    if operator.chars().all(|ch| ch.is_ascii_alphabetic()) {
        find_keyword_top_level(input, operator)
    } else {
        let mut state = ScanState::default();
        let bytes = input.as_bytes();
        for index in 0..input.len().saturating_sub(operator.len() - 1) {
            state.update(bytes[index] as char);
            if state.top_level() && input[index..].starts_with(operator) {
                return Some(index);
            }
        }
        None
    }
}

fn find_char_top_level(input: &str, target: char) -> Option<usize> {
    let mut state = ScanState::default();
    for (index, ch) in input.char_indices() {
        if state.top_level() && ch == target {
            return Some(index);
        }
        state.update(ch);
    }
    None
}

fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut state = ScanState::default();
    for (index, ch) in input.char_indices() {
        state.update(ch);
        if state.top_level() && ch == delimiter {
            parts.push(input[start..index].trim());
            start = index + ch.len_utf8();
        }
    }
    parts.push(input[start..].trim());
    parts
}

fn matching_delimiter(input: &str, start: usize, open: char, close: char) -> Result<usize, String> {
    let mut depth = 0i32;
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in input[start..].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Ok(start + offset);
            }
        }
    }
    Err(format!("unclosed delimiter {open}"))
}

fn skip_ws(input: &str, cursor: &mut usize) {
    while *cursor < input.len() && input.as_bytes()[*cursor].is_ascii_whitespace() {
        *cursor += 1;
    }
}

fn boundary(input: &str, start: usize, len: usize) -> bool {
    let before =
        start == 0 || !is_identifier_char(input.as_bytes()[start.saturating_sub(1)] as char);
    let after_index = start + len;
    let after =
        after_index >= input.len() || !is_identifier_char(input.as_bytes()[after_index] as char);
    before && after
}

#[derive(Default)]
struct ScanState {
    paren: i32,
    bracket: i32,
    brace: i32,
    quote: Option<char>,
    escaped: bool,
}

impl ScanState {
    fn update(&mut self, ch: char) {
        if let Some(active_quote) = self.quote {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == active_quote {
                self.quote = None;
            }
            return;
        }
        match ch {
            '\'' | '"' => self.quote = Some(ch),
            '(' => self.paren += 1,
            ')' => self.paren -= 1,
            '[' => self.bracket += 1,
            ']' => self.bracket -= 1,
            '{' => self.brace += 1,
            '}' => self.brace -= 1,
            _ => {}
        }
    }

    fn top_level(&self) -> bool {
        self.paren == 0
            && self.bracket == 0
            && self.brace == 0
            && self.quote.is_none()
            && !self.escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_match_projection() {
        let query = parse_query(
            "MATCH (a:Person {name: $name})-[r:KNOWS]->(b) WHERE b.rank >= 2 RETURN b.name AS name ORDER BY name LIMIT 5",
        )
        .unwrap();
        assert!(!query.is_write());
    }

    #[test]
    fn create_and_match_round_trip() {
        let mut core = GraphCore::new();
        let params = CypherParams::new();
        let created = execute_autocommit(
            &mut core,
            "CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'}) RETURN a, b",
            &params,
        )
        .unwrap();
        assert_eq!(created.summary.nodes_created, 2);
        assert_eq!(created.summary.relationships_created, 1);

        let result = execute_autocommit(
            &mut core,
            "MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name AS source, type(r) AS rel, b.name AS target",
            &params,
        )
        .unwrap();
        assert_eq!(result.keys, vec!["source", "rel", "target"]);
        assert_eq!(result.records.len(), 1);
    }

    #[test]
    fn update_and_delete_crud_round_trip() {
        let mut core = GraphCore::new();
        let params = CypherParams::new();
        execute_autocommit(
            &mut core,
            "CREATE (a:Person {external_id: 'alice', name: 'Alice', obsolete: 'x'})-[r:KNOWS {old: 'yes'}]->(b:Person {external_id: 'bob'}) RETURN a",
            &params,
        )
        .unwrap();

        let updated = execute_autocommit(
            &mut core,
            "MATCH (a:Person {external_id: 'alice'})-[r:KNOWS]->(b) SET a.name = 'Alicia', a += {rank: 2, obsolete: null}, a:Researcher, r.weight = 0.5 REMOVE a:Person, r.old RETURN a.name AS name, a.rank AS rank, labels(a) AS labels, r.weight AS weight",
            &params,
        )
        .unwrap();
        assert_eq!(updated.summary.properties_set, 3);
        assert_eq!(updated.summary.properties_removed, 2);
        assert_eq!(updated.summary.labels_added, 1);
        assert_eq!(updated.summary.labels_removed, 1);
        assert_eq!(updated.records.len(), 1);
        assert_eq!(core.get_node_id("alice"), Some(0));
        assert_eq!(core.nodes_with_label("Researcher"), vec![0]);
        assert_eq!(core.nodes_with_label("Person"), vec![1]);

        let error = execute_autocommit(
            &mut core,
            "MATCH (a {external_id: 'alice'}) DELETE a",
            &params,
        )
        .unwrap_err();
        assert!(error.contains("relationships"));
        assert_eq!(core.node_count(), 2);
        assert_eq!(core.edge_count(), 1);

        let deleted = execute_autocommit(
            &mut core,
            "MATCH (a {external_id: 'alice'})-[r:KNOWS]->(b) DELETE r, a",
            &params,
        )
        .unwrap();
        assert_eq!(deleted.summary.nodes_deleted, 1);
        assert_eq!(deleted.summary.relationships_deleted, 1);
        assert_eq!(core.node_count(), 1);
        assert_eq!(core.edge_count(), 0);
    }

    #[test]
    fn detach_delete_and_external_id_update_work() {
        let mut core = GraphCore::new();
        let params = CypherParams::from([(
            "updates".to_string(),
            CypherValue::Map(BTreeMap::from([
                ("name".to_string(), CypherValue::String("Alice".to_string())),
                ("active".to_string(), CypherValue::Bool(true)),
            ])),
        )]);
        execute_autocommit(
            &mut core,
            "CREATE (a {external_id: 'a'})-[:LINK]->(b {external_id: 'b'})",
            &params,
        )
        .unwrap();
        let result = execute_autocommit(
            &mut core,
            "MATCH (a {external_id: 'a'}) SET a.external_id = 'alice', a += $updates RETURN a.external_id AS id, a.name AS name",
            &params,
        )
        .unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(core.get_node_id("a"), None);
        assert_eq!(core.get_node_id("alice"), Some(0));

        let deleted = execute_autocommit(
            &mut core,
            "MATCH (a {external_id: 'alice'}) DETACH DELETE a",
            &params,
        )
        .unwrap();
        assert_eq!(deleted.summary.nodes_deleted, 1);
        assert_eq!(deleted.summary.relationships_deleted, 1);
    }

    #[test]
    fn duplicate_label_updates_count_unique_changes() {
        let mut core = GraphCore::new();
        core.add_node(Some("n".to_string()), Vec::new(), PropertyMap::new())
            .unwrap();
        let added = execute_autocommit(
            &mut core,
            "MATCH (n {external_id: 'n'}) SET n:A:A RETURN labels(n) AS labels",
            &CypherParams::new(),
        )
        .unwrap();
        assert_eq!(added.summary.labels_added, 1);
        assert_eq!(core.nodes_with_label("A"), vec![0]);
        let removed = execute_autocommit(
            &mut core,
            "MATCH (n {external_id: 'n'}) REMOVE n:A:A RETURN labels(n) AS labels",
            &CypherParams::new(),
        )
        .unwrap();
        assert_eq!(removed.summary.labels_removed, 1);
        assert!(core.nodes_with_label("A").is_empty());
    }

    #[test]
    fn crud_parser_rejects_unsupported_combinations_and_readonly_writes() {
        let core = GraphCore::new();
        assert!(parse_query("MATCH (n) DELETE n RETURN n")
            .unwrap_err()
            .contains("not supported"));
        assert!(parse_query("MATCH (n) SET n.x = 1 DELETE n")
            .unwrap_err()
            .contains("cannot be combined"));
        assert!(parse_query("MATCH (n) REMOVE n.x SET n.x = 1")
            .unwrap_err()
            .contains("cannot follow"));
        assert!(parse_query("MATCH (n) RETURN n UNION MATCH (m) DELETE m")
            .unwrap_err()
            .contains("only supports read"));
        let mut mutable = GraphCore::new();
        mutable
            .add_node(Some("n".to_string()), Vec::new(), PropertyMap::new())
            .unwrap();
        assert!(execute_autocommit(
            &mut mutable,
            "MATCH (n {external_id: 'n'}) REMOVE n.external_id",
            &CypherParams::new(),
        )
        .unwrap_err()
        .contains("cannot be removed"));
        let error =
            execute_snapshot(&core, "MATCH (n) SET n.x = 1", &CypherParams::new()).unwrap_err();
        assert!(error.contains("cannot execute write"));
    }

    #[test]
    fn snapshot_rejects_writes() {
        let core = GraphCore::new();
        let error =
            execute_snapshot(&core, "CREATE (n) RETURN n", &CypherParams::new()).unwrap_err();
        assert!(error.contains("cannot execute write"));
    }
}
