# Copyright (c) Meta Platforms, Inc. and affiliates.
# All rights reserved.

# This source code is licensed under the license found in the
# LICENSE file in the root directory of this source tree.

from .sequential import Sequential
from .geometric import Crop, HorizontalFlip, Identity, Perspective, Resize, Rotate
from .valuemetric import JPEG, Brightness, Contrast, GaussianBlur, Hue, MedianFilter, Saturation
from .video import H264, H264rgb, H265


def get_validation_augs_subset(
    is_video: bool = False
) -> list:
    """
    Get the validation augmentations.
    """
    if is_video:
        # less augs for videos because more expensive
        augs = [
            (Identity(),          [0]),  # No parameters needed for identity
            (HorizontalFlip(),    [0]),  # No parameters needed for flip
            (Crop(),              [0.71]),  # size ratio
            (Brightness(),        [0.5]),
            (H264(),              [40]),
            (Sequential(H264(), Crop(), Brightness()), [(40, 0.71, 0.5)]),
        ]
    else:
        augs = [
            (Identity(),          [0]),  # No parameters needed for identity
            (HorizontalFlip(),    [0]),  # No parameters needed for flip
            (Crop(),              [0.71]),  # size ratio
            (Brightness(),        [0.5]),
            (JPEG(),              [60]),
            (Sequential(JPEG(), Crop(), Brightness()), [(60, 0.71, 0.5)]),
        ]
    return augs


def get_validation_augs(
    is_video: bool = False
) -> list:
    """
    Get the validation augmentations.
    """
    if is_video:
        # less augs for videos because more expensive
        augs = [
            (Identity(),          [0]),  # No parameters needed for identity
            (HorizontalFlip(),    [0]),  # No parameters needed for flip
            (Rotate(),            [10, 90]),  # (min_angle, max_angle)
            (Resize(),            [0.55, 0.71]),  # size ratio
            (Crop(),              [0.55, 0.71]),  # size ratio
            (Perspective(),       [0.5]),  # distortion_scale
            (Brightness(),        [0.5, 1.5]),
            (Contrast(),          [0.5, 1.5]),
            (Saturation(),        [0.5, 1.5]),
            (Hue(),               [0.25]),
            (JPEG(),              [40]),
            (GaussianBlur(),      [9]),
            (MedianFilter(),      [9]),
            (H264(),              [30, 40, 50, 60]),
            (H264rgb(),           [30, 40, 50, 60]),
            (H265(),              [30, 40, 50]),  # crf > 50 is not valid
            (Sequential(H264(), Crop(), Brightness()), [(30, 0.71, 0.5)]),
            (Sequential(H264(), Crop(), Brightness()), [(40, 0.71, 0.5)]),
            (Sequential(H264(), Crop(), Brightness()), [(50, 0.71, 0.5)]),
        ]
    else:
        augs = [
            (Identity(),          [0]),  
            (HorizontalFlip(),    [0]),  
            (Rotate(),            [5, 10, 30, 45, 90]),  
            (Resize(),            [0.32, 0.45, 0.55, 0.63, 0.71, 0.77, 0.84, 0.89, 0.95, 1.00]),  # size ratio, such 0.1 increment in area ratio
            (Crop(),              [0.32, 0.45, 0.55, 0.63, 0.71, 0.77, 0.84, 0.89, 0.95, 1.00]),  
            (Perspective(),       [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]),  # distortion_scale
            (Brightness(),        [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]),
            (Contrast(),          [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]),
            (Hue(),               [-0.4, -0.3, -0.2, -0.1, 0.0, 0.1, 0.2, 0.3, 0.4, 0.5]),
            (JPEG(),              [40, 50, 60, 70, 80, 90]),
            (GaussianBlur(),      [3, 5, 9, 13, 17]),
            (MedianFilter(),      [3, 5, 9, 13, 17]),
            (Sequential(JPEG(), Crop(), Brightness()), [(40, 0.71, 0.5)]),
            (Sequential(JPEG(), Crop(), Brightness()), [(60, 0.71, 0.5)]),
            (Sequential(JPEG(), Crop(), Brightness()), [(80, 0.71, 0.5)]),
        ]
    return augs
