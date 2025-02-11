"""
Flask server that accepts video files, applies watermarks, and returns the processed videos.
"""

import os
import io
import torch
import ffmpeg
import numpy as np
import uuid
import subprocess
from flask import Flask, request, Response, jsonify
from requests_toolbelt import MultipartEncoder
import json
from torchcodec.decoders import VideoDecoder
from torchaudio.io import StreamWriter

device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
torch.set_grad_enabled(False)

import videoseal
from videoseal.utils.display import save_video_audio_to_mp4

# Initialize Flask app
app = Flask(__name__)

# Initialize models
video_model = videoseal.load("videoseal")
video_model.eval()
video_model.to(device)


def process_video_chunk(chunk, message_bits=None):
    """Process a video chunk by applying watermarks."""

    clip_tensor = chunk.data.float() / 255.0
    size_bytes = clip_tensor.element_size() * clip_tensor.nelement()
    outputs = video_model.embed(clip_tensor, msgs=message_bits, is_video=True)
    processed_clip = outputs["imgs_w"]
    processed_clip = (processed_clip * 255.0).byte()
    return processed_clip

def embed_video(input_path, output_path, chunk_size=8):
    """Process a video using streaming to handle memory efficiently."""
    # Generate random message bits to use for all chunks
    message_bits = torch.randint(0, 2, (1, 96), device=device)
    message_bits_list = message_bits.cpu().numpy().flatten().tolist()


    decoder = VideoDecoder(input_path, device='cuda', dimension_order='NCHW')
    width = decoder.metadata.width
    height = decoder.metadata.height
    frame_rate = decoder.metadata.average_fps
    num_frames = decoder.metadata.num_frames

    stream = StreamWriter(output_path, format="mp4")
    stream.add_video_stream(frame_rate=frame_rate, width=width, height=height, hw_accel="cuda:0", encoder="h264_nvenc", encoder_format="rgb0")
    with stream.open():
        for i in range(0, len(decoder), chunk_size):
            frames = decoder[i : i + chunk_size]
            processed_frames = process_video_chunk(frames, message_bits.clone())
            stream.write_video_chunk(0, processed_frames)

    return output_path, message_bits_list

def analyze_video_chunk(chunk):
    """Analyze a video chunk to extract watermark bits."""
    # Convert chunk to tensor and normalize to [0, 1]
    clip_tensor = chunk.data.float() / 255.0

    # Extract watermark bits
    outputs = video_model.detect(clip_tensor, is_video=True)
    output_bits = outputs["preds"][:, 1:]  # exclude the first which may be used for detection
    
    return output_bits

def analyze_video(input_path, chunk_size=8):
    """Process a video using streaming to extract watermark bits."""
    # Get video info
    probe = ffmpeg.probe(input_path)
    video_info = next(s for s in probe['streams'] if s['codec_type'] == 'video')
    width = int(video_info['width'])
    height = int(video_info['height'])
    num_frames = int(video_info['nb_frames'])

    decoder = VideoDecoder(input_path, device='cuda', dimension_order='NCHW')
    soft_msgs = []
    for i in range(0, len(decoder), chunk_size):
        frames = decoder[i : i + chunk_size]
        output_bits = analyze_video_chunk(frames)
        soft_msgs.append(output_bits)


    all_msgs = torch.cat(soft_msgs, dim=0)
    # Average across frames to get final bit predictions
    avg_msg = all_msgs.mean(dim=0)
    # Convert to binary predictions
    final_bits = avg_msg.cpu().numpy().tolist()
    return final_bits

@app.route('/process_video', methods=['POST'])
def process_video_file():
    """Handle video upload and processing."""
    if 'video' not in request.files:
        return 'No video file uploaded', 400

    video_file = request.files['video']
    if not video_file.filename:
        return 'No video file selected', 400

    # Create temporary files for input and output
    temp_input = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"temp_input_{uuid.uuid4()}.mp4")
    temp_output = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"temp_output_{uuid.uuid4()}.mp4")
    temp_output2 = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"temp_output2_{uuid.uuid4()}.mp4")
    
    try:
        # Save uploaded file
        video_file.save(temp_input)
        
        # Process video using streaming
        _, message_bits = embed_video(temp_input, temp_output)
        audiostream = ffmpeg.input(temp_input)
        videostream = ffmpeg.input(temp_output)
        process3 = (
            ffmpeg
            .output(
                videostream.video,
                audiostream.audio,
                temp_output2,
                vcodec='copy',
                acodec='copy'
            )
            .overwrite_output()
            .run_async(pipe_stderr=subprocess.PIPE)
        )
        combine_result = process3.wait()

        
        m = MultipartEncoder(fields={
            'video': open(temp_output2 if combine_result == 0 else temp_output, 'rb'),
            'message_bits': json.dumps(message_bits)
        })

        def generate():
            # Read in chunks (adjust chunk size as needed)
            chunk_size = 4096
            for chunk in iter(lambda: m.read(chunk_size), b''):
                    yield chunk


        return Response(generate(), mimetype=m.content_type)

    finally:
        # Clean up temporary files
        for path in [temp_input, temp_output, temp_output2]:
            if os.path.exists(path):
                try:
                    os.remove(path)
                except OSError:
                    pass

@app.route('/analyze_video', methods=['POST'])
def analyze_video_file():
    """Handle video upload and watermark extraction."""
    if 'video' not in request.files:
        return 'No video file uploaded', 400

    video_file = request.files['video']
    if not video_file.filename:
        return 'No video file selected', 400

    # Create temporary file for input
    temp_input = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"temp_input_{uuid.uuid4()}.mp4")
    
    try:
        # Save uploaded file
        video_file.save(temp_input)
        
        # Extract watermark bits
        extracted_bits = analyze_video(temp_input)
        
        return jsonify({
            'extracted_bits': extracted_bits
        })

    except Exception as e:
        return str(e), 500
    
    finally:
        # Clean up temporary files
        if os.path.exists(temp_input):
            os.remove(temp_input)

if __name__ == "__main__":
    app.run(host='0.0.0.0', port=8001)