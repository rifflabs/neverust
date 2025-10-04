#!/usr/bin/env python3
"""
Neverust Network Visualizer - Real-time D3.js dashboard
"""

import os
import time
import threading
from flask import Flask, render_template
from flask_socketio import SocketIO, emit
from prometheus_api_client import PrometheusConnect
import requests

app = Flask(__name__)
app.config['SECRET_KEY'] = 'neverust-viz-secret'
socketio = SocketIO(app, cors_allowed_origins="*")

PROMETHEUS_URL = os.getenv('PROMETHEUS_URL', 'http://prometheus:9090')
NUM_NODES = int(os.getenv('NUM_NODES', 50))

prom = PrometheusConnect(url=PROMETHEUS_URL, disable_ssl=True)

def get_node_metrics():
    """Fetch metrics for all nodes from Prometheus."""
    try:
        # Block count per node
        block_counts = prom.custom_query('neverust_block_count')
        # Block bytes per node
        block_bytes = prom.custom_query('neverust_block_bytes')
        # Uptime per node
        uptimes = prom.custom_query('neverust_uptime_seconds')

        metrics = {}
        for metric in block_counts:
            instance = metric['metric'].get('instance', 'unknown')
            node_id = instance.split(':')[0]
            metrics[node_id] = {
                'block_count': int(metric['value'][1]),
                'block_bytes': 0,
                'uptime': 0,
                'health': 'up'
            }

        for metric in block_bytes:
            instance = metric['metric'].get('instance', 'unknown')
            node_id = instance.split(':')[0]
            if node_id in metrics:
                metrics[node_id]['block_bytes'] = int(metric['value'][1])

        for metric in uptimes:
            instance = metric['metric'].get('instance', 'unknown')
            node_id = instance.split(':')[0]
            if node_id in metrics:
                metrics[node_id]['uptime'] = int(metric['value'][1])

        return metrics
    except Exception as e:
        print(f"Error fetching metrics: {e}")
        return {}

def get_network_topology():
    """Generate network topology from active nodes."""
    metrics = get_node_metrics()

    nodes = []
    links = []

    # Bootstrap node
    nodes.append({
        'id': 'bootstrap',
        'name': 'Bootstrap',
        'type': 'bootstrap',
        'block_count': metrics.get('bootstrap', {}).get('block_count', 0),
        'block_bytes': metrics.get('bootstrap', {}).get('block_bytes', 0)
    })

    # Worker nodes
    for i in range(1, NUM_NODES + 1):
        node_id = f'node{i}'
        node_metrics = metrics.get(node_id, {})

        nodes.append({
            'id': node_id,
            'name': f'Node {i}',
            'type': 'marketplace' if i % 3 == 0 else 'altruistic',
            'block_count': node_metrics.get('block_count', 0),
            'block_bytes': node_metrics.get('block_bytes', 0),
            'health': node_metrics.get('health', 'down')
        })

        # Create links to bootstrap
        links.append({
            'source': node_id,
            'target': 'bootstrap',
            'value': 1
        })

        # Create links between some nodes (mesh topology)
        if i % 5 == 0 and i > 5:
            links.append({
                'source': node_id,
                'target': f'node{i-5}',
                'value': 0.5
            })

    return {'nodes': nodes, 'links': links}

def broadcast_metrics():
    """Background thread to broadcast metrics every 2 seconds."""
    while True:
        try:
            topology = get_network_topology()
            metrics = get_node_metrics()

            # Calculate aggregate stats
            total_blocks = sum(m.get('block_count', 0) for m in metrics.values())
            total_bytes = sum(m.get('block_bytes', 0) for m in metrics.values())
            active_nodes = len([m for m in metrics.values() if m.get('health') == 'up'])

            data = {
                'topology': topology,
                'stats': {
                    'total_blocks': total_blocks,
                    'total_bytes': total_bytes,
                    'active_nodes': active_nodes,
                    'total_nodes': NUM_NODES + 1  # +1 for bootstrap
                }
            }

            socketio.emit('metrics_update', data)
        except Exception as e:
            print(f"Error broadcasting metrics: {e}")

        time.sleep(2)

@app.route('/')
def index():
    """Serve the visualization dashboard."""
    return render_template('index.html', num_nodes=NUM_NODES)

@socketio.on('connect')
def handle_connect():
    """Handle client connection."""
    print('Client connected')
    # Send initial data
    topology = get_network_topology()
    emit('metrics_update', {'topology': topology, 'stats': {}})

@socketio.on('disconnect')
def handle_disconnect():
    """Handle client disconnection."""
    print('Client disconnected')

if __name__ == '__main__':
    # Start metrics broadcast thread
    metrics_thread = threading.Thread(target=broadcast_metrics, daemon=True)
    metrics_thread.start()

    # Run Flask app
    socketio.run(app, host='0.0.0.0', port=8888, debug=True, allow_unsafe_werkzeug=True)
