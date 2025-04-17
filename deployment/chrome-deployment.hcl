job "chrome-deployment"  {
  datacenters = ["dc1"]

  group "chrome-deployment" {
    count = 10
    network {
      port "cdp_port" {}
      port "tzafonwright_port" {}
    }
    update {
      max_parallel      = 0
      progress_deadline = "10m"
      health_check      = "checks"
      stagger           = "30s"
    }
    task "server" {
      driver = "docker"

      config {
        image = "ghcr.io/tzafon/browser-container:latest"
        ports = ["cdp_port", "tzafonwright_port"]
        args = [
            "/app/browser-container",
            "--chrome-binary-path", "/chrome-headless-shell-linux64/chrome-headless-shell",
            // Change this to how it is exposed from the cluster
            "--instance-manager", "https://instance-manager:50052",
            "--cdp-port", "${NOMAD_PORT_cdp_port}",
            "--ip-address", "${NOMAD_IP_cdp_port}",
            "--tzafonwright-port", "${NOMAD_PORT_tzafonwright_port}"
        ]
        
      }
      resources {
        cpu    = 500
        memory = 1600
        memory_max = 2200
      }
      restart {
        attempts = 10000
        delay    = "0s"
        interval = "0m1s"
        mode     = "delay"
      }
    }
  }
}