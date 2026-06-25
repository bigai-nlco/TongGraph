# Probabilistic Inference

Use variables, CPDs, factor tables, and evidence when graph records contain
uncertainty. TongGraph compiles an active subgraph and runs finite discrete
belief propagation locally.

```python
from tonggraph import Graph

graph = Graph()

sensor = graph.add_node("sensor:1", labels=["Sensor"])
alert = graph.add_node("alert:1", labels=["Alert"])
graph.add_edge(sensor, alert, "INFLUENCES")

sensor_ok = graph.add_variable(
    "binary",
    owner_id=sensor,
    prior={"p": 0.95},
)
alert_active = graph.add_variable(
    "binary",
    owner_id=alert,
    prior={"p": 0.05},
)

graph.add_cpd(
    alert_active,
    [sensor_ok],
    [
        0.8,
        0.2,
        0.1,
        0.9,
    ],
)

active = graph.compile_active_subgraph(
    [alert_active],
    evidence={sensor_ok: "false"},
    radius=1,
)
result = graph.belief_propagation(
    [alert_active],
    evidence={sensor_ok: "false"},
    radius=1,
    persist=True,
)

print(active)
print(result["converged"], result["max_residual"])
print(graph.posterior(alert_active))
```

Belief propagation is approximate on loopy graphs. Use the returned convergence
metadata and traces when interpreting results.
