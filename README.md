# C2PA Video Watermarking Service

A service that adds C2PA information and invisible watermarks to videos. The system consists of three main components:

1. **Frontend**: React-based web interface for video upload and processing
2. **Rust Backend**: Handles video chunking and C2PA signature application
3. **Python Backend**: Applies invisible watermarks to video chunks using CUDA acceleration

## Prerequisites

- Docker and Docker Compose
- NVIDIA GPU with CUDA support
- NVIDIA Container Toolkit

## Setup

1. Make sure your OS has a working CUDA installation. This is operating system dependent. This detailed guide from [nvidia](https://docs.nvidia.com/cuda/cuda-installation-guide-linux/index.html) may help you. If you encounter CUDA related problems, you might have to also install the [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html)

2. Build and start the services:
```bash
docker compose up --build
```
3. Check the logs to make sure all containers are running. The python backend may take a few seconds to start.

The services will be available at:
- Frontend: http://localhost:3000
- Rust Backend: http://localhost:8000
- Python Backend: http://localhost:5000

## Project Structure

```
.
├── docker-compose.yml
├── frontend/
│   ├── Dockerfile
│   └── ...
├── rust-backend/
│   ├── Dockerfile
│   └── ...
└── python-backend/
    ├── Dockerfile
    ├── requirements.txt
    └── ...
```
