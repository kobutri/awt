# Copyright (c) Meta Platforms, Inc. and affiliates.
# All rights reserved.

# This source code is licensed under the license found in the
# LICENSE file in the root directory of this source tree.

"""
Test with:
    python -m videoseal.augmentation.augmenter
"""

import os

import torch
from torch import nn

from .geometric import Crop, HorizontalFlip, Identity, Perspective, Resize, Rotate
from .valuemetric import JPEG, Brightness, Contrast, GaussianBlur, Hue, MedianFilter, Saturation
from .video import VideoCompressorAugmenter, H264
from .masks import get_mask_embedder

name2aug = {
    'rotate': Rotate,
    'resize': Resize,
    'crop': Crop,
    'perspective': Perspective,
    'hflip': HorizontalFlip,
    'identity': Identity,
    'jpeg': JPEG,
    'gaussian_blur': GaussianBlur,
    'median_filter': MedianFilter,
    'brightness': Brightness,
    'contrast': Contrast,
    'saturation': Saturation,
    'hue': Hue,
    'video_compression': VideoCompressorAugmenter,
    'h264': H264,
}

def get_dummy_augmenter():
    """
    An augmenter that does nothing.
    """
    return Augmenter(
        augs = {'identity': 1},
        augs_params = {},
        masks = {'kind': None}
    )

class Augmenter(nn.Module):
    """
    Augments the watermarked image.
    """

    def __init__(
        self,
        masks: dict,
        augs: dict,
        augs_params: dict,
        num_augs: int = 1,
        **kwargs: dict
    ) -> None:
        """
        Args:
            masks: (dict) The parameters for the mask embedder. \
                E.g. {'kind': None}
            augs: (dict) The augmentations to apply. \
                E.g. {'identity': 4, 'resize': 1, 'crop': 1}
            augs_params: (dict) The parameters for each augmentation. \
                E.g. {'resize': {'min_size': 0.7, 'max_size': 1.5}, 'crop': {'min_size': 0.7, 'max_size': 1.0}}
            num_augs: (int) The number of augmentations to apply.
            **kwargs: (dict) Additional arguments for the mask embedder.
        """
        super(Augmenter, self).__init__()

        # create mask embedder, not used by default
        self.mask_embedder = get_mask_embedder(
            **masks  # contains: kind, invert_proba, etc.
        )

        # create augs
        self.augs, self.aug_probs = self.parse_augmentations(
            augs=augs,
            augs_params=augs_params
        )
        self.num_augs = num_augs

    def parse_augmentations(
        self,
        augs: dict[str, float],
        augs_params: dict[str, dict[str, float]],
    ):
        """
        Parse the post augmentations into a list of augmentations.
        Args:
            augs: (dict) The augmentations to apply.
                e.g. {'identity': 4, 'resize': 1, 'crop': 1}
            augs_params: (dict) The parameters for each augmentation.
                e.g. {'resize': {'min_size': 0.7, 'max_size': 1.5}, 'crop': {'min_size': 0.7, 'max_size': 1.0}}
        """
        augmentations = []
        probs = []
        # parse each augmentation
        for aug_name in augs.keys():
            aug_prob = float(augs[aug_name])
            aug_params = augs_params[aug_name] if aug_name in augs_params else {
            }
            try:
                selected_aug = name2aug[aug_name](**aug_params)
            except KeyError:
                raise ValueError(
                    f"Augmentation {aug_name} not found. Add it in name2aug.")
            augmentations.append(selected_aug)
            probs.append(aug_prob)
        # normalize probabilities
        total_prob = sum(probs)
        probs = [prob / total_prob for prob in probs]
        return augmentations, torch.tensor(probs)

    def augment(self, image, mask=None, is_video=False, do_resize=True):
        if not is_video:  # replace video compression with identity
            augs = [aug if aug.__class__.__name__ != 'VideoCompressorAugmenter' else Identity() for aug in self.augs]
        else:
            augs = self.augs
        index = torch.multinomial(self.aug_probs, 1).item()
        selected_aug = augs[index]
        if not do_resize:
            image, mask = selected_aug(image, mask)
        else:
            h, w = image.shape[-2:]
            image, mask = selected_aug(image, mask)
            if image.shape[-2:] != (h, w):
                image = nn.functional.interpolate(image, size=(
                    h, w), mode='bilinear', align_corners=False, antialias=True)
                mask = nn.functional.interpolate(mask, size=(
                    h, w), mode='bilinear', align_corners=False, antialias=True)
        return image, mask, selected_aug.__class__.__name__

    def forward(
        self,
        imgs_w: torch.Tensor,
        imgs: torch.Tensor,
        masks: torch.Tensor = None,
        is_video=True
    ) -> torch.Tensor:
        """
        Args:
            imgs: (torch.Tensor) Batched images with shape BxCxHxW
            imgs_w: (torch.Tensor) Batched watermarked images with shape BxCxHxW
            masks: (torch.Tensor) Batched masks with shape Bx1xHxW
        Returns:
            imgs_aug: (torch.Tensor) The augmented watermarked images.
            mask_targets: (torch.Tensor) The mask targets, at ones where the watermark is present.
        """
        if self.training:
            # create mask targets
            mask_targets = self.mask_embedder(
                imgs_w, masks=masks).to(imgs_w.device)
            # watermark masking
            imgs_aug = imgs_w * mask_targets + imgs * (1 - mask_targets)
            # image augmentations
            selected_augs = []
            for _ in range(self.num_augs):
                imgs_aug, mask_targets, selected_aug_ = self.augment(
                    imgs_aug, mask_targets, is_video, do_resize=False)
                selected_augs.append(selected_aug_)
            selected_aug = "+".join(selected_augs)
            return imgs_aug, mask_targets, selected_aug
        else:
            # no mask
            mask_targets = torch.ones_like(imgs_w)[:, 0:1, :, :]
            # image augmentations
            selected_augs = []
            for _ in range(self.num_augs):
                imgs_aug, mask_targets, selected_aug_ = self.augment(
                    imgs_aug, mask_targets, is_video, do_resize=False)
                selected_augs.append(selected_aug_)
            return imgs_aug, mask_targets, selected_aug

    def __repr__(self) -> str:
        # print the augmentations and their probabilities
        augs = [aug.__class__.__name__ for aug in self.augs]
        return f"Augmenter(augs={augs}, probs={self.aug_probs})"


if __name__ == "__main__":

    from PIL import Image
    from torchvision import transforms
    from torchvision.utils import save_image

    # Define the augmentations and their parameters
    augs = {
        'identity': 1,
        'rotate': 1,
        'resize': 1,
        'crop': 1,
        'perspective': 1,
        'hue': 1,
        'jpeg': 1,
        'gaussian_blur': 1,
        'median_filter': 1,
    }
    augs_params = {
        'resize': {'min_size': 0.7, 'max_size': 1.5},
        'crop': {'min_size': 0.5, 'max_size': 0.7},
        'rotate': {'min_angle': -10, 'max_angle': 10},
        'perspective': {'min_distortion_scale': 0.1, 'max_distortion_scale': 0.5},
        'jpeg': {'min_quality': 40, 'max_quality': 80},
        'hue': {'min_factor': -0.5, 'max_factor': 0.5},
        'gaussian_blur': {'min_kernel_size': 3, 'max_kernel_size': 17},
        'median_filter': {'min_kernel_size': 3, 'max_kernel_size': 9},
    }

    # Create a batch of images
    img = Image.open(f"assets/imgs/1.jpg").convert("RGB")
    imgs = transforms.ToTensor()(img).unsqueeze(0)
    imgs_w = imgs.clone()

    # Create an instance of the Augmenter class
    masks = {'kind': None}
    augmenter = Augmenter(
        masks=masks,
        augs=augs,
        augs_params=augs_params,
        num_augs=2
    )
    print("Augmenter:", augmenter)

    # Apply the augmentations to the images and save
    output_dir = "outputs/augmentations"
    os.makedirs(output_dir, exist_ok=True)
    for ii in range(10):
        imgs_aug, mask_targets, selected_aug = augmenter(imgs_w, imgs)
        save_image(imgs_aug.clamp(0, 1), os.path.join(output_dir, f"imgs_aug_{ii}_{selected_aug}.png"))
