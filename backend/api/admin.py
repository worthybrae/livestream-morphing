"""
Admin API endpoints for managing stylization configuration.
Allows real-time adjustments to video processing parameters.
"""
from fastapi import APIRouter, HTTPException
from fastapi.responses import FileResponse
from pydantic import BaseModel
from typing import Optional
from pathlib import Path
import json
import os

router = APIRouter()

# Path to config file
CONFIG_PATH = os.path.join(os.path.dirname(__file__), "..", "core", "config.json")

class StylizationConfig(BaseModel):
    """Configuration model for image processing parameters"""
    bilateral_diameter: int = 15
    bilateral_sigma_color: int = 80
    bilateral_sigma_space: int = 80
    gaussian_blur_size: int = 9
    quantization_levels: int = 8
    morph_kernel_size: int = 3
    edge_blend_factor: float = 0.2
    psychedelic_amplitude: float = 0.01
    psychedelic_frequency: float = 20.0
    process_every_nth_frame: int = 3

# In-memory config (loaded on startup)
current_config = StylizationConfig()

@router.get("/config", response_model=StylizationConfig)
async def get_config():
    """Get current stylization configuration"""
    return current_config

@router.post("/config", response_model=StylizationConfig)
async def update_config(config: StylizationConfig):
    """Update stylization configuration"""
    global current_config
    current_config = config

    # Save to file for persistence
    try:
        os.makedirs(os.path.dirname(CONFIG_PATH), exist_ok=True)
        with open(CONFIG_PATH, 'w') as f:
            json.dump(config.dict(), f, indent=2)
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to save config: {e}")

    return current_config

@router.post("/config/reset")
async def reset_config():
    """Reset configuration to defaults"""
    global current_config
    current_config = StylizationConfig()

    # Delete config file
    if os.path.exists(CONFIG_PATH):
        os.remove(CONFIG_PATH)

    return current_config

@router.get("/status")
async def get_status():
    """Get current processor status"""
    from backend.core.processor import get_processor_status
    return get_processor_status()

@router.post("/stream-url")
async def update_stream_url(request: dict):
    """Update the stream base URL"""
    if "url" not in request:
        raise HTTPException(status_code=400, detail="Missing 'url' field")

    stream_url = request["url"]

    # Store in processor module
    from backend.core import processor
    processor.STREAM_BASE_URL = stream_url

    print(f"🔄 Stream URL updated to: {stream_url}")

    return {
        "success": True,
        "url": stream_url
    }

@router.get("/frames/{segment_id}/{frame_number}.jpg")
async def get_frame(segment_id: str, frame_number: int):
    """Serve a specific processed frame image"""
    # Get the base directory where frames are stored
    from backend.core.processor import FRAMES_DIR

    frame_path = Path(FRAMES_DIR) / segment_id / f"{frame_number}.jpg"

    if not frame_path.exists():
        raise HTTPException(status_code=404, detail="Frame not found")

    return FileResponse(
        frame_path,
        media_type='image/jpeg',
        headers={
            'Cache-Control': 'max-age=3600',
            'Access-Control-Allow-Origin': '*'
        }
    )

@router.get("/code")
async def get_processing_code(file: str = "cpp"):
    """Get the C++ or Python processing code for editing"""

    if file == "cpp":
        cpp_code_path = Path(__file__).parent.parent / "core" / "fast_processor.cpp"

        if cpp_code_path.exists():
            with open(cpp_code_path, 'r') as f:
                cpp_code = f.read()
            return {
                "code": cpp_code,
                "language": "cpp",
                "file_name": "fast_processor.cpp"
            }
        else:
            raise HTTPException(status_code=404, detail="C++ file not found")

    elif file == "python":
        # Fallback to Python if C++ doesn't exist
        code_path = Path(__file__).parent.parent / "core" / "image_processing.py"
        if not code_path.exists():
            raise HTTPException(status_code=404, detail="Processing code not found")

        with open(code_path, 'r') as f:
            full_code = f.read()

        # Extract just the process_frame_fast_blobs function
        import ast
        try:
            tree = ast.parse(full_code)
            for node in ast.walk(tree):
                if isinstance(node, ast.FunctionDef) and node.name == 'process_frame_fast_blobs':
                    # Get the function source
                    function_lines = full_code.split('\n')[node.lineno - 1:node.end_lineno]
                    function_code = '\n'.join(function_lines)
                    return {
                        "code": function_code,
                        "language": "python",
                        "file_name": "image_processing.py"
                    }
        except Exception as e:
            print(f"Error extracting function: {e}")

        # Fallback to full code if extraction fails
        return {
            "code": full_code,
            "language": "python",
            "file_name": "image_processing.py"
        }

@router.post("/code")
async def update_processing_code(request: dict):
    """Update C++ processing code and rebuild extension"""
    if "code" not in request:
        raise HTTPException(status_code=400, detail="Missing 'code' field")

    new_code = request["code"]
    language = request.get("language", "cpp")

    if language == "cpp":
        # Update C++ code
        cpp_path = Path(__file__).parent.parent / "core" / "fast_processor.cpp"

        try:
            # Backup current code
            backup_path = cpp_path.with_suffix('.cpp.bak')
            if cpp_path.exists():
                with open(cpp_path, 'r') as f:
                    with open(backup_path, 'w') as bf:
                        bf.write(f.read())

            # Write new code
            with open(cpp_path, 'w') as f:
                f.write(new_code)

            # Rebuild the C++ extension
            import subprocess
            base_dir = Path(__file__).parent.parent.parent

            print("🔨 Rebuilding C++ extension...")
            result = subprocess.run(
                ["python3", "setup.py", "build_ext", "--inplace"],
                cwd=str(base_dir),
                capture_output=True,
                text=True
            )

            if result.returncode != 0:
                # Restore backup on failure
                if backup_path.exists():
                    with open(backup_path, 'r') as bf:
                        with open(cpp_path, 'w') as f:
                            f.write(bf.read())

                raise HTTPException(
                    status_code=500,
                    detail=f"C++ build failed:\n{result.stderr}"
                )

            print("✅ C++ extension rebuilt successfully!")

            # Enable C++ processor
            from backend.core import image_processing
            image_processing.USE_CPP = True

            # Reimport to get new version
            try:
                import fast_processor
                import importlib
                importlib.reload(fast_processor)
            except Exception as e:
                print(f"Warning: Could not reload C++ module: {e}")

            return {
                "success": True,
                "message": "C++ code updated and rebuilt successfully",
                "requires_restart": False
            }

        except HTTPException:
            raise
        except Exception as e:
            # Restore backup on error
            if backup_path.exists():
                with open(backup_path, 'r') as bf:
                    with open(cpp_path, 'w') as f:
                        f.write(bf.read())
            raise HTTPException(status_code=500, detail=f"Failed to update C++ code: {str(e)}")

    else:
        # Python code update (existing logic)
        code_path = Path(__file__).parent.parent / "core" / "image_processing.py"

        try:
            # Read the full file
            with open(code_path, 'r') as f:
                full_code = f.read()

            # Backup the current code
            backup_path = code_path.with_suffix('.py.bak')
            with open(backup_path, 'w') as f:
                f.write(full_code)

            # Find and replace the process_frame_fast_blobs function
            import ast
            tree = ast.parse(full_code)
            lines = full_code.split('\n')

            for node in ast.walk(tree):
                if isinstance(node, ast.FunctionDef) and node.name == 'process_frame_fast_blobs':
                    # Replace the function in the file
                    start_line = node.lineno - 1
                    end_line = node.end_lineno

                    # Build new file content
                    new_lines = lines[:start_line] + new_code.split('\n') + lines[end_line:]
                    new_full_code = '\n'.join(new_lines)

                    # Write the new code
                    with open(code_path, 'w') as f:
                        f.write(new_full_code)

                    break

            # Reload the module
            import importlib
            from backend.core import image_processing
            importlib.reload(image_processing)

            # Update the reference in processor module
            from backend.core import processor
            processor.process_frame_fast_blobs = image_processing.process_frame_fast_blobs

            # Mark the current latest segment - next segment after this will have new effects
            last_segment = max(processor.ready_segments) if processor.ready_segments else None
            first_new_segment = last_segment + 1 if last_segment else None

            print(f"🔄 Code updated - new effects will apply starting from segment {first_new_segment}")

            return {
                "success": True,
                "message": "Code updated and reloaded",
                "buffer_cleared": False,
                "first_new_segment": first_new_segment
            }

        except Exception as e:
            # Restore backup on error
            if backup_path.exists():
                with open(backup_path, 'r') as f:
                    backup_code = f.read()
                with open(code_path, 'w') as f:
                    f.write(backup_code)

            raise HTTPException(status_code=500, detail=f"Failed to update code: {str(e)}")

# Load config from file on module import
def load_config():
    global current_config
    if os.path.exists(CONFIG_PATH):
        try:
            with open(CONFIG_PATH, 'r') as f:
                data = json.load(f)
                current_config = StylizationConfig(**data)
        except Exception as e:
            print(f"Failed to load config: {e}")

load_config()
