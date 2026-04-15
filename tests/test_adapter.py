import io
import pytest
from PIL import Image
from service.adapters.image_adapter import load_image_from_path, load_image_from_bytes, ImageLoadError


def test_load_image_from_path(tmp_path):
    # Create a dummy image
    img = Image.new("RGB", (100, 100), color="red")
    img_path = tmp_path / "test_img.jpg"
    img.save(img_path)

    loaded_img = load_image_from_path(str(img_path))
    assert loaded_img.size == (100, 100)
    assert loaded_img.mode == "RGB"


def test_load_image_from_path_not_found():
    with pytest.raises(ImageLoadError) as exc:
        load_image_from_path("non_existent.jpg")
    assert "not found" in str(exc.value).lower()


def test_load_image_from_path_invalid(tmp_path):
    invalid_path = tmp_path / "invalid.jpg"
    invalid_path.write_text("not an image")
    with pytest.raises(ImageLoadError) as exc:
        load_image_from_path(str(invalid_path))
    assert "cannot decode" in str(exc.value).lower()


def test_load_image_from_bytes():
    img = Image.new("RGB", (50, 50), color="blue")
    buf = io.BytesIO()
    img.save(buf, format="JPEG")
    img_bytes = buf.getvalue()

    loaded_img = load_image_from_bytes(img_bytes)
    assert loaded_img.size == (50, 50)
    assert loaded_img.mode == "RGB"


def test_load_image_from_bytes_invalid():
    with pytest.raises(ImageLoadError) as exc:
        load_image_from_bytes(b"not an image")
    assert "cannot decode" in str(exc.value).lower()


def test_load_image_from_bytes_empty():
    with pytest.raises(ImageLoadError) as exc:
        load_image_from_bytes(b"")
    assert "empty" in str(exc.value).lower()
