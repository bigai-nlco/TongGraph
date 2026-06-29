"""In-process operations metrics for TongGraph Server."""

from __future__ import annotations

import threading
import time
from collections import Counter
from dataclasses import dataclass, field
from typing import Any


@dataclass
class RequestMetrics:
    start_time: float = field(default_factory=time.time)
    total_requests: int = 0
    error_requests: int = 0
    in_flight_requests: int = 0
    status_counts: Counter[str] = field(default_factory=Counter)
    route_counts: Counter[str] = field(default_factory=Counter)
    method_counts: Counter[str] = field(default_factory=Counter)
    latency_total_ms: float = 0.0
    latency_max_ms: float = 0.0
    _lock: threading.Lock = field(default_factory=threading.Lock)

    def begin(self) -> None:
        with self._lock:
            self.in_flight_requests += 1

    def record(self, method: str, route: str, status_code: int, latency_ms: float) -> None:
        status = str(status_code)
        route_key = f"{method.upper()} {route}"
        with self._lock:
            self.total_requests += 1
            self.in_flight_requests = max(0, self.in_flight_requests - 1)
            if status_code >= 400:
                self.error_requests += 1
            self.status_counts[status] += 1
            self.route_counts[route_key] += 1
            self.method_counts[method.upper()] += 1
            self.latency_total_ms += latency_ms
            self.latency_max_ms = max(self.latency_max_ms, latency_ms)

    def finish_unrecorded(self) -> None:
        with self._lock:
            self.in_flight_requests = max(0, self.in_flight_requests - 1)

    def snapshot(self) -> dict[str, Any]:
        with self._lock:
            average = self.latency_total_ms / self.total_requests if self.total_requests else 0.0
            return {
                "start_time": self.start_time,
                "uptime_seconds": max(0.0, time.time() - self.start_time),
                "total_requests": self.total_requests,
                "error_requests": self.error_requests,
                "in_flight_requests": self.in_flight_requests,
                "status_counts": dict(self.status_counts),
                "route_counts": dict(self.route_counts),
                "method_counts": dict(self.method_counts),
                "latency_ms": {
                    "total": self.latency_total_ms,
                    "max": self.latency_max_ms,
                    "average": average,
                },
            }
