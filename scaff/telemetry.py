"""Telemetry and error tracking for scaff."""

import json
import platform
from pathlib import Path
from datetime import datetime
from typing import Dict, Any, Optional
from dataclasses import dataclass, asdict


@dataclass
class ErrorReport:
    """Error tracking record"""
    timestamp: str
    error_type: str
    error_message: str
    stack_trace: Optional[str] = None
    context: Dict[str, Any] = None
    user_action: Optional[str] = None
    resolved: bool = False


@dataclass
class PerformanceMetric:
    """Performance metric record"""
    timestamp: str
    operation: str
    duration_ms: float
    success: bool
    metadata: Dict[str, Any] = None


class Telemetry:
    """Collect and report telemetry data"""
    
    def __init__(self, telemetry_dir: Optional[Path] = None):
        """Initialize telemetry"""
        self.telemetry_dir = telemetry_dir or Path.home() / ".scaff" / "telemetry"
        self.telemetry_dir.mkdir(parents=True, exist_ok=True)
        
        self.errors_file = self.telemetry_dir / "errors.jsonl"
        self.performance_file = self.telemetry_dir / "performance.jsonl"
        self.events_file = self.telemetry_dir / "events.jsonl"
    
    def report_error(
        self,
        error_type: str,
        error_message: str,
        stack_trace: Optional[str] = None,
        context: Optional[Dict[str, Any]] = None,
        user_action: Optional[str] = None,
    ) -> None:
        """Report an error"""
        report = ErrorReport(
            timestamp=datetime.now().isoformat(),
            error_type=error_type,
            error_message=error_message,
            stack_trace=stack_trace,
            context=context or {},
            user_action=user_action,
            resolved=False,
        )
        
        try:
            with open(self.errors_file, "a") as f:
                f.write(json.dumps(asdict(report)) + "\n")
        except (IOError, OSError):
            pass
    
    def record_performance(
        self,
        operation: str,
        duration_ms: float,
        success: bool = True,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Record performance metric"""
        metric = PerformanceMetric(
            timestamp=datetime.now().isoformat(),
            operation=operation,
            duration_ms=duration_ms,
            success=success,
            metadata=metadata or {},
        )
        
        try:
            with open(self.performance_file, "a") as f:
                f.write(json.dumps(asdict(metric)) + "\n")
        except (IOError, OSError):
            pass
    
    def log_event(
        self,
        event_name: str,
        properties: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Log an event"""
        event = {
            "timestamp": datetime.now().isoformat(),
            "event": event_name,
            "properties": properties or {},
        }
        
        try:
            with open(self.events_file, "a") as f:
                f.write(json.dumps(event) + "\n")
        except (IOError, OSError):
            pass
    
    def get_error_summary(self) -> Dict[str, Any]:
        """Get summary of errors"""
        if not self.errors_file.exists():
            return {"total_errors": 0, "error_types": {}}
        
        error_types: Dict[str, int] = {}
        total_errors = 0
        
        try:
            with open(self.errors_file, "r") as f:
                for line in f:
                    try:
                        error = json.loads(line)
                        total_errors += 1
                        error_type = error.get("error_type", "unknown")
                        error_types[error_type] = error_types.get(error_type, 0) + 1
                    except json.JSONDecodeError:
                        continue
        except (IOError, OSError):
            pass
        
        return {
            "total_errors": total_errors,
            "error_types": error_types,
            "most_common": max(error_types, key=error_types.get) if error_types else None,
        }
    
    def get_performance_stats(self, operation: Optional[str] = None) -> Dict[str, Any]:
        """Get performance statistics"""
        if not self.performance_file.exists():
            return {}
        
        metrics_by_op: Dict[str, list] = {}
        
        try:
            with open(self.performance_file, "r") as f:
                for line in f:
                    try:
                        metric = json.loads(line)
                        op = metric.get("operation", "unknown")
                        
                        if operation is None or op == operation:
                            if op not in metrics_by_op:
                                metrics_by_op[op] = []
                            metrics_by_op[op].append(metric)
                    except json.JSONDecodeError:
                        continue
        except (IOError, OSError):
            pass
        
        # Calculate stats for each operation
        stats = {}
        for op, metrics in metrics_by_op.items():
            durations = [m.get("duration_ms", 0) for m in metrics]
            successful = sum(1 for m in metrics if m.get("success", True))
            
            stats[op] = {
                "count": len(metrics),
                "successful": successful,
                "failed": len(metrics) - successful,
                "avg_duration_ms": sum(durations) / len(durations) if durations else 0,
                "min_duration_ms": min(durations) if durations else 0,
                "max_duration_ms": max(durations) if durations else 0,
            }
        
        return stats
    
    def get_system_info(self) -> Dict[str, str]:
        """Get system information"""
        return {
            "platform": platform.system(),
            "platform_release": platform.release(),
            "python_version": platform.python_version(),
            "machine": platform.machine(),
        }
