#!/usr/bin/env python3
"""
Generate docker-compose.yml for N-node Neverust cluster with monitoring.
"""

import sys
import yaml

def generate_compose(num_nodes=50):
    """Generate docker-compose configuration for N nodes + monitoring stack."""

    compose = {
        'version': '3.8',
        'services': {},
        'networks': {
            'neverust-net': {
                'driver': 'bridge',
                'ipam': {
                    'config': [{'subnet': '172.25.0.0/16'}]
                }
            }
        },
        'volumes': {
            'prometheus-data': {},
            'grafana-data': {}
        }
    }

    # Bootstrap node
    compose['services']['bootstrap'] = {
        'build': '..',
        'container_name': 'neverust-bootstrap',
        'hostname': 'bootstrap',
        'networks': {
            'neverust-net': {
                'ipv4_address': '172.25.0.10'
            }
        },
        'ports': ['9080:8080'],  # Expose API for getting bootstrap info
        'command': [
            'start',
            '--mode', 'altruistic',
            '--listen-port', '8070',
            '--disc-port', '8090',
            '--api-port', '8080',
            '--data-dir', '/data',
            '--log-level', 'info'
        ],
        'healthcheck': {
            'test': ['CMD', 'curl', '-f', 'http://localhost:8080/health'],
            'interval': '5s',
            'timeout': '3s',
            'retries': 3
        }
    }

    # Worker nodes
    for i in range(1, num_nodes + 1):
        node_ip = f'172.25.{(i // 254) + 1}.{(i % 254) + 1}'

        compose['services'][f'node{i}'] = {
            'build': '..',
            'container_name': f'neverust-node{i}',
            'hostname': f'node{i}',
            'networks': {
                'neverust-net': {
                    'ipv4_address': node_ip
                }
            },
            'depends_on': ['bootstrap'],
            'command': [
                'start',
                '--mode', 'altruistic' if i % 3 != 0 else 'marketplace',  # 1/3 marketplace nodes
                '--listen-port', '8070',
                '--disc-port', '8090',
                '--api-port', '8080',
                '--data-dir', '/data',
                '--log-level', 'warn'  # Reduce log noise
            ],
            'environment': {
                'BOOTSTRAP_NODE': 'bootstrap:8080'
            },
            'healthcheck': {
                'test': ['CMD', 'curl', '-f', 'http://localhost:8080/health'],
                'interval': '10s',
                'timeout': '3s',
                'retries': 3,
                'start_period': '10s'
            }
        }

        # Expose API for first 5 nodes (for testing)
        if i <= 5:
            compose['services'][f'node{i}']['ports'] = [f'{9080 + i}:8080']

    # Prometheus
    compose['services']['prometheus'] = {
        'image': 'prom/prometheus:latest',
        'container_name': 'prometheus',
        'networks': ['neverust-net'],
        'ports': ['9090:9090'],
        'volumes': [
            './prometheus.yml:/etc/prometheus/prometheus.yml',
            'prometheus-data:/prometheus'
        ],
        'command': [
            '--config.file=/etc/prometheus/prometheus.yml',
            '--storage.tsdb.path=/prometheus',
            '--web.console.libraries=/usr/share/prometheus/console_libraries',
            '--web.console.templates=/usr/share/prometheus/consoles',
            '--web.enable-lifecycle'
        ]
    }

    # Grafana
    compose['services']['grafana'] = {
        'image': 'grafana/grafana:latest',
        'container_name': 'grafana',
        'networks': ['neverust-net'],
        'ports': ['3000:3000'],
        'volumes': [
            'grafana-data:/var/lib/grafana',
            './grafana/provisioning:/etc/grafana/provisioning',
            './grafana/dashboards:/var/lib/grafana/dashboards'
        ],
        'environment': {
            'GF_SECURITY_ADMIN_PASSWORD': 'neverust',
            'GF_USERS_ALLOW_SIGN_UP': 'false'
        },
        'depends_on': ['prometheus']
    }

    # Network Visualizer
    compose['services']['visualizer'] = {
        'build': './visualizer',
        'container_name': 'neverust-viz',
        'networks': ['neverust-net'],
        'ports': ['8888:8888'],
        'environment': {
            'PROMETHEUS_URL': 'http://prometheus:9090',
            'NUM_NODES': str(num_nodes)
        },
        'depends_on': ['prometheus']
    }

    return compose

if __name__ == '__main__':
    num_nodes = int(sys.argv[1]) if len(sys.argv) > 1 else 50

    compose = generate_compose(num_nodes)

    with open('docker-compose.yml', 'w') as f:
        yaml.dump(compose, f, default_flow_style=False, sort_keys=False)

    print(f"✅ Generated docker-compose.yml for {num_nodes} nodes + monitoring stack")
    print(f"📊 Services: Bootstrap + {num_nodes} workers + Prometheus + Grafana + Visualizer")
    print(f"🌐 Grafana: http://localhost:3000 (admin/neverust)")
    print(f"📈 Prometheus: http://localhost:9090")
    print(f"🎨 Visualizer: http://localhost:8888")
