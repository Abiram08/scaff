#!/usr/bin/env python
"""Build and package scaff as a standalone executable."""

import subprocess
import sys
from pathlib import Path


def run_command(cmd: list[str], description: str) -> bool:
    """Run a command and report results"""
    print(f"\n{'='*60}")
    print(f"[*] {description}")
    print(f"{'='*60}")
    
    try:
        result = subprocess.run(cmd, check=True)
        print(f"[OK] {description} - Complete")
        return True
    except subprocess.CalledProcessError as e:
        print(f"[ERROR] {description} failed with code {e.returncode}")
        return False


def main():
    """Build and package scaff"""
    project_root = Path(__file__).parent
    
    # Step 1: Run tests
    if not run_command(
        [sys.executable, "-m", "pytest", "tests/", "-v"],
        "Running all tests"
    ):
        print("\n[ERROR] Tests failed. Aborting build.")
        return False
    
    # Step 2: Install dependencies
    if not run_command(
        [sys.executable, "-m", "pip", "install", "-e", "."],
        "Installing scaff in development mode"
    ):
        print("\n[ERROR] Installation failed.")
        return False
    
    # Step 3: Build wheel and sdist
    if not run_command(
        [sys.executable, "-m", "pip", "install", "build"],
        "Installing build tools"
    ):
        print("\n[ERROR] Failed to install build tools.")
        return False
    
    if not run_command(
        [sys.executable, "-m", "build"],
        "Building wheel and source distribution"
    ):
        print("\n[ERROR] Build failed.")
        return False
    
    # Step 4: Create .pyz archive (optional)
    print(f"\n{'='*60}")
    print("[*] Creating Python executable (.pyz)")
    print(f"{'='*60}")
    
    try:
        import shutil
        
        # Create a simple __main__.py for the .pyz
        pyz_dir = project_root / ".pyz_build"
        pyz_dir.mkdir(exist_ok=True)
        
        # Copy scaff package
        shutil.copytree(
            project_root / "scaff",
            pyz_dir / "scaff",
            dirs_exist_ok=True,
        )
        
        # Create __main__.py
        main_py = pyz_dir / "__main__.py"
        main_py.write_text(
            "from scaff.main import app\n"
            "if __name__ == '__main__':\n"
            "    app()\n"
        )
        
        # Create .pyz
        pyz_path = project_root / "dist" / "scaff.pyz"
        pyz_path.parent.mkdir(exist_ok=True)
        
        shutil.make_archive(
            str(pyz_path.with_suffix("")),
            "zip",
            pyz_dir,
        )
        
        # Rename to .pyz
        pyz_zip = pyz_path.with_suffix(".zip")
        if pyz_zip.exists():
            pyz_zip.rename(pyz_path)
            
            # Add shebang
            pyz_content = pyz_path.read_bytes()
            pyz_path.write_bytes(
                b"#!/usr/bin/env python3\n" + pyz_content
            )
            
            # Make executable (Unix only)
            import sys as _sys
            if _sys.platform != "win32":
                pyz_path.chmod(0o755)
            
            print(f"[OK] Created executable: {pyz_path}")
        
        # Cleanup
        shutil.rmtree(pyz_dir, ignore_errors=True)
        
    except Exception as e:
        print(f"[WARNING] Failed to create .pyz: {e}")
    
    # Final summary
    print(f"\n{'='*60}")
    print("[OK] Build complete!")
    print(f"{'='*60}")
    print("\nDistribution files:")
    dist_dir = project_root / "dist"
    if dist_dir.exists():
        for file in sorted(dist_dir.iterdir()):
            print(f"  - {file.name}")
    
    print("\nInstall with:")
    print(f"  pip install {project_root}/dist/scaff-*.whl")
    print("\nOr run directly:")
    print(f"  python {project_root}/dist/scaff.pyz --help")
    
    return True


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
