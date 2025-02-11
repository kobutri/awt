FROM jrottenberg/ffmpeg:6.1-nvidia2204 AS ffmpeg-stage
FROM ghcr.io/astral-sh/uv:bookworm-slim AS builder
ENV UV_COMPILE_BYTECODE=1 UV_LINK_MODE=copy

# Configure the Python directory so it is consistent
ENV UV_PYTHON_INSTALL_DIR=/python

# Only use the managed Python version
ENV UV_PYTHON_PREFERENCE=only-managed

# Install Python before the project for caching
RUN uv python install 3.12

WORKDIR /app
ADD https://dl.fbaipublicfiles.com/videoseal/checkpoint.pth ./checkpoints/checkpoint.pth
RUN --mount=type=cache,target=/root/.cache/uv \
    --mount=type=bind,source=uv.lock,target=uv.lock \
    --mount=type=bind,source=pyproject.toml,target=pyproject.toml \
    uv sync --frozen --no-install-project --no-dev
ADD . /app
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --frozen --no-dev


# Stage 2: Use your base image
FROM nvidia/cuda:12.4.1-cudnn-runtime-ubuntu22.04

# Install missing system libraries (libgomp and libexpat are missing in your output)
RUN apt-get update && \
    apt-get install -y python3-opencv && \
    rm -rf /var/lib/apt/lists/*

# Copy ffmpeg-related libraries from the ffmpeg-stage
COPY --from=ffmpeg-stage /usr/local/lib/ /usr/local/lib/

# Copy ffmpeg and ffprobe binaries from the ffmpeg-stage
COPY --from=ffmpeg-stage /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg-stage /usr/local/bin/ffprobe /usr/local/bin/ffprobe

# Copy missing system libraries from the ffmpeg-stage
COPY --from=ffmpeg-stage /lib/x86_64-linux-gnu/libgomp.so.* /lib/x86_64-linux-gnu/
COPY --from=ffmpeg-stage /lib/x86_64-linux-gnu/libexpat.so.* /lib/x86_64-linux-gnu/

# ---------------------------------------------------------
# Copy missing libraries from /usr/local/lib
# ---------------------------------------------------------
# We use wildcards to cover the specific versions.
COPY --from=ffmpeg-stage /usr/local/lib/libavdevice.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libavfilter.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libavformat.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libavcodec.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libpostproc.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libswresample.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libswscale.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libavutil.so.*   /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libass.so.*       /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libvidstab.so.*     /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libzmq.so.*         /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libzimg.so.*        /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libfontconfig.so.*  /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libfreetype.so.*    /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libbluray.so.*      /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libsrt.so.*         /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libvpx.so.*         /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libaribb24.so.*     /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libopencore-amrwb.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libaom.so.*         /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libfdk-aac.so.*     /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libmp3lame.so.*       /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libopencore-amrnb.so.* /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libopenjp2.so.*      /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libopus.so.*         /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libtheoraenc.so.*     /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libtheoradec.so.*     /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libvorbis.so.*        /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libvorbisenc.so.*     /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libwebp.so.*          /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libx264.so.*          /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libx265.so.*          /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libxvidcore.so.*       /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libkvazaar.so.*       /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libfribidi.so.*       /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libxml2.so.*          /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libpng16.so.*         /usr/local/lib/
COPY --from=ffmpeg-stage /usr/local/lib/libogg.so.*            /usr/local/lib/

# Set the library search path so the copied libraries are found at runtime
ENV LD_LIBRARY_PATH=/usr/local/lib:/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH


WORKDIR /app

COPY --from=builder --chown=python:python /python /python
COPY --from=builder --chown=app:app /app /app
ENV PATH="/app/.venv/bin:$PATH"

EXPOSE 8001

ENV CUDA_LAUNCH_BLOCKING=1
ENV FLASK_APP=service
ENV FLASK_ENV=development

CMD ["python", "inference_av.py"]
