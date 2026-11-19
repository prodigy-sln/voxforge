---
name: voxel-asset-creation
description: >-
  Use this skill when you need to generate 3D voxel models (.mcvox) and export them for use in games.
  This skill sets up a pipeline where a primary agent creates the model, and a separate independent review agent evaluates the generated PNG previews to ensure visual quality and coherence.
---

# Voxel Asset Creation Pipeline

When creating voxel assets for games using VoxForge, follow this multi-agent workflow to ensure the highest visual quality.

## Pipeline Steps

### 1. Author the Model
* Write or modify the `.mcvox` document in your workspace.
* For the format specification and instructions on how to use the `.mcvox` format, refer to [`docs/format.md`](../../../docs/format.md) in this repository.
* Use the materials defined in the project's materials directory (`assets/materials` by default).

### 2. Generate PNG Previews
* Before finalizing, render preview images of the model.
* Run the preview command (adjust paths as needed):
  `voxforge preview models/fox.mcvox --out previews/fox.png --materials assets/materials`

### 3. Invoke an Independent Visual Reviewer
* **Do NOT judge the model yourself.** You must spawn an independent subagent specifically for visual review.
* Use the `invoke_subagent` tool to spawn a subagent. Give it the `Role` of "Voxel Art Director".
* Example Subagent Prompt:
  > "I have generated a preview of a voxel asset at `previews/fox.png`. Use the `view_file` tool to examine this image. Evaluate its silhouette, color coherence, readability, and proportions. Do NOT look at the source code. Provide your critique and specify any changes needed to improve the design."

### 4. Iterate
* If the review agent suggests changes, edit the `.mcvox` file, regenerate the preview, and send a message back to the reviewer asking them to evaluate the new version.
* Repeat this process until the Art Director approves the design.

### 5. Finalize and Export
* Once the review agent approves, you can export the final assets (`.vox`, `.obj` or `.glb`) using the `voxforge export` command.
  `voxforge export models/fox.mcvox --out exports/fox.glb --materials assets/materials`
