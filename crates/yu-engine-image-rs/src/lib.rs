mod output;

use image::{ColorType, DynamicImage, ImageError as NativeImageError, ImageFormat, ImageReader};
use output::OutputTransaction;
use std::path::Path;
use yu_capability_image::{
    ConvertRequest, ConvertResult, CropRequest, CropResult, ImageEngine, ImageInfo,
    ImageOperationError, ResizeRequest, ResizeResult, RotateRequest, RotateResult,
};

pub const ENGINE_ID: &str = "raster-rs";

#[derive(Debug, Default, Clone, Copy)]
pub struct RustImageEngine;

impl ImageEngine for RustImageEngine {
    fn id(&self) -> &'static str {
        ENGINE_ID
    }

    fn info(&self, path: &Path) -> Result<ImageInfo, ImageOperationError> {
        let (image, format) = open_image(path)?;
        let color = image.color();

        Ok(ImageInfo {
            path: path.to_string_lossy().into_owned(),
            format: format_name(format),
            width: image.width(),
            height: image.height(),
            color_type: format!("{color:?}").to_lowercase(),
            bit_depth: bits_per_channel(color),
            channels: color.channel_count(),
            has_alpha: color.has_alpha(),
        })
    }

    fn resize(&self, request: &ResizeRequest) -> Result<ResizeResult, ImageOperationError> {
        let output_format = supported_output_format(&request.output)?;
        let transaction =
            OutputTransaction::prepare(&request.output, &request.output_policy, request.dry_run)?;

        let (image, _) = open_image(&request.input)?;
        let source_width = image.width();
        let source_height = image.height();
        let (width, height) =
            target_dimensions(source_width, source_height, request.width, request.height)?;

        if !request.dry_run {
            let resized = image.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
            transaction.publish(&resized, output_format)?;
        }

        Ok(ResizeResult {
            input: request.input.to_string_lossy().into_owned(),
            output: request.output.to_string_lossy().into_owned(),
            source_width,
            source_height,
            width,
            height,
            format: format_name(output_format),
            dry_run: request.dry_run,
            replaced: transaction.replaced(),
            would_replace: transaction.would_replace(),
        })
    }

    fn crop(&self, request: &CropRequest) -> Result<CropResult, ImageOperationError> {
        let format = supported_output_format(&request.output)?;
        let transaction =
            OutputTransaction::prepare(&request.output, &request.output_policy, request.dry_run)?;
        let (image, _) = open_image(&request.input)?;
        let (source_width, source_height) = (image.width(), image.height());

        if request.width == 0 || request.height == 0 {
            return Err(ImageOperationError::invalid_input(
                "crop width and height must be greater than zero",
            ));
        }
        let right = request.x.checked_add(request.width);
        let bottom = request.y.checked_add(request.height);
        if !matches!(right, Some(x) if x <= source_width)
            || !matches!(bottom, Some(y) if y <= source_height)
        {
            return Err(ImageOperationError::invalid_input(
                "crop rectangle extends outside source image bounds",
            ));
        }

        if !request.dry_run {
            let cropped = image.crop_imm(request.x, request.y, request.width, request.height);
            transaction.publish(&cropped, format)?;
        }
        Ok(CropResult {
            input: request.input.to_string_lossy().into_owned(),
            output: request.output.to_string_lossy().into_owned(),
            source_width,
            source_height,
            x: request.x,
            y: request.y,
            width: request.width,
            height: request.height,
            format: format_name(format),
            dry_run: request.dry_run,
            replaced: transaction.replaced(),
            would_replace: transaction.would_replace(),
        })
    }

    fn rotate(&self, request: &RotateRequest) -> Result<RotateResult, ImageOperationError> {
        // Only right-angle geometry is part of M4a. Arbitrary-angle
        // resampling and background handling require a separate contract.
        if !matches!(request.degrees, 90 | 180 | 270) {
            return Err(ImageOperationError::invalid_input(
                "rotation must be 90, 180, or 270 degrees clockwise",
            ));
        }
        let format = supported_output_format(&request.output)?;
        let transaction =
            OutputTransaction::prepare(&request.output, &request.output_policy, request.dry_run)?;
        let (image, _) = open_image(&request.input)?;
        let (source_width, source_height) = (image.width(), image.height());
        let (width, height) = if request.degrees == 180 {
            (source_width, source_height)
        } else {
            (source_height, source_width)
        };
        if !request.dry_run {
            let rotated = match request.degrees {
                90 => image.rotate90(),
                180 => image.rotate180(),
                270 => image.rotate270(),
                _ => unreachable!("degrees validated above"),
            };
            transaction.publish(&rotated, format)?;
        }
        Ok(RotateResult {
            input: request.input.to_string_lossy().into_owned(),
            output: request.output.to_string_lossy().into_owned(),
            source_width,
            source_height,
            degrees: request.degrees,
            width,
            height,
            format: format_name(format),
            dry_run: request.dry_run,
            replaced: transaction.replaced(),
            would_replace: transaction.would_replace(),
        })
    }

    fn convert(&self, request: &ConvertRequest) -> Result<ConvertResult, ImageOperationError> {
        let format = supported_output_format(&request.output)?;
        let transaction =
            OutputTransaction::prepare(&request.output, &request.output_policy, request.dry_run)?;
        let (image, source_format) = open_image(&request.input)?;
        if !request.dry_run {
            transaction.publish(&image, format)?;
        }
        Ok(ConvertResult {
            input: request.input.to_string_lossy().into_owned(),
            output: request.output.to_string_lossy().into_owned(),
            source_format: format_name(source_format),
            format: format_name(format),
            width: image.width(),
            height: image.height(),
            dry_run: request.dry_run,
            replaced: transaction.replaced(),
            would_replace: transaction.would_replace(),
        })
    }
}

fn supported_output_format(path: &Path) -> Result<ImageFormat, ImageOperationError> {
    let format = ImageFormat::from_path(path).map_err(|error| {
        ImageOperationError::unsupported(format!(
            "cannot infer a supported output format from {}: {error}",
            path.display()
        ))
    })?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(ImageOperationError::unsupported(
            "built-in image output supports PNG, JPEG, and WebP only",
        ));
    }
    Ok(format)
}

fn open_image(path: &Path) -> Result<(DynamicImage, ImageFormat), ImageOperationError> {
    let reader = ImageReader::open(path).map_err(|error| {
        ImageOperationError::invalid_input(format!("cannot open {}: {error}", path.display()))
    })?;

    let reader = reader.with_guessed_format().map_err(|error| {
        ImageOperationError::invalid_input(format!(
            "cannot detect image format for {}: {error}",
            path.display()
        ))
    })?;

    let format = reader.format().ok_or_else(|| {
        ImageOperationError::unsupported(format!(
            "unsupported or unknown image format: {}",
            path.display()
        ))
    })?;

    let image = reader.decode().map_err(map_decode_error)?;
    Ok((image, format))
}

fn target_dimensions(
    source_width: u32,
    source_height: u32,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<(u32, u32), ImageOperationError> {
    if source_width == 0 || source_height == 0 {
        return Err(ImageOperationError::invalid_input(
            "source image has zero width or height",
        ));
    }

    if width == Some(0) || height == Some(0) {
        return Err(ImageOperationError::invalid_input(
            "resize dimensions must be greater than zero",
        ));
    }

    match (width, height) {
        (Some(width), Some(height)) => Ok((width, height)),
        (Some(width), None) => {
            let height = scaled_dimension(source_height, width, source_width)?;
            Ok((width, height))
        }
        (None, Some(height)) => {
            let width = scaled_dimension(source_width, height, source_height)?;
            Ok((width, height))
        }
        (None, None) => Err(ImageOperationError::invalid_input(
            "at least one of --width or --height is required",
        )),
    }
}

fn scaled_dimension(
    source_dimension: u32,
    target_dimension: u32,
    reference_dimension: u32,
) -> Result<u32, ImageOperationError> {
    let numerator = u64::from(source_dimension) * u64::from(target_dimension);
    let rounded = (numerator + u64::from(reference_dimension) / 2) / u64::from(reference_dimension);
    let rounded = rounded.max(1);

    u32::try_from(rounded).map_err(|_| {
        ImageOperationError::invalid_input("calculated resize dimension exceeds supported range")
    })
}

fn bits_per_channel(color: ColorType) -> u8 {
    let channels = u16::from(color.channel_count()).max(1);
    (color.bits_per_pixel() / channels) as u8
}

fn format_name(format: ImageFormat) -> String {
    format!("{format:?}").to_lowercase()
}

fn map_decode_error(error: NativeImageError) -> ImageOperationError {
    match error {
        NativeImageError::Unsupported(_) => {
            ImageOperationError::unsupported(format!("image format is not supported: {error}"))
        }
        _ => ImageOperationError::invalid_input(format!("cannot decode image: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_only_preserves_aspect_ratio() {
        assert_eq!(
            target_dimensions(400, 200, Some(100), None).unwrap(),
            (100, 50)
        );
    }

    #[test]
    fn height_only_preserves_aspect_ratio() {
        assert_eq!(
            target_dimensions(400, 200, None, Some(50)).unwrap(),
            (100, 50)
        );
    }

    #[test]
    fn two_dimensions_are_exact() {
        assert_eq!(
            target_dimensions(400, 200, Some(123), Some(77)).unwrap(),
            (123, 77)
        );
    }

    #[test]
    fn missing_dimensions_are_rejected() {
        let error = target_dimensions(400, 200, None, None).unwrap_err();
        assert_eq!(
            error.kind,
            yu_capability_image::ImageErrorKind::InvalidInput
        );
    }
}
