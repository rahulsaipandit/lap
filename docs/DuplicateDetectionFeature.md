# Duplicate Detection Feature Documentation

This document provides a detailed breakdown of the DedupPane_2.vue component, which implements a user interface for detecting, reviewing, and managing duplicate files in the Lap photo manager application. The component handles exact hash-based duplicate detection, filtering, selection, and deletion operations.

## Overview

The DedupPane_2 component is a Vue.js component that presents a two-panel interface for duplicate file management:

- **Left Panel**: List of duplicate groups with summary information
- **Right Panel**: Detailed view of files within a selected group

Key features:
- Real-time duplicate scanning with progress tracking
- Media type filtering (All, Image, RAW, Video)
- Album-specific duplicate selection and deletion
- Keep/protect mechanism for files
- Batch deletion with reclaimable space calculation
- Folder comparison: Compare two arbitrary directories (source vs. target) instead of library-based scanning
- Custom ignore patterns with wildcard support and pre-loaded defaults
- Multiple comparison criteria (name, size, timestamp with tolerance, binary content)
- Dual tree views for hierarchical folder comparison
- Manual folder selection via browse dialogs
- Target-specific deletion with confirmation and recycle bin usage

## UI Structure

### Header Section
- **Title**: "Duplicate Review" with subtitle "Exact hash matches from LAP albums"
- **Refresh Button**: Triggers new duplicate scan. Files deleted outside the app are detected during the scan (path no longer exists while its folder is still readable) and removed from the results along with their cached hashes; files on unreachable drives are left alone.
- **Close Button**: Emits close event

### Statistics Cards (3-column grid)
- **Groups**: Total number of duplicate groups
- **Duplicates**: Total number of duplicate files
- **Reclaim**: Total reclaimable space from duplicates

### Loading State
Displays a spinner with "Scanning..." text when duplicate detection is in progress.

### Main Content Area (Grid Layout: 35% left, 65% right)

#### Left Panel: Duplicate Groups List
- Scrollable list of duplicate groups
- Each group item shows:
  - Thumbnail of keep file
  - Group number and media type badge
  - Album summary (up to 3 album names)
  - Reclaimable space and file count
- Click to select group for detailed view

#### Right Panel: Group Details
- **Group Header**: Group number, file count, reclaimable space, compare/split button, select all button
- **File List**: Scrollable list of all files in group
  - Each file shows:
    - Checkbox for selection (or lock icon if protected)
    - Thumbnail
    - Filename with keep/protected badges
    - Folder path
    - File details: size, type, dimensions, creation date
    - "Set Keep" button (if applicable)

### Control Panel
- **Match Criteria**: Shows "Exact hash", "Same file size", "Backend verified"
- **Media Type Filter**: 4 buttons (All, Image, RAW, Video)
- **Delete Duplicates From Album**:
  - Dropdown to select target album
  - "Select" button to auto-select duplicates in that album
  - Protection note about files outside selected album
- **Action Buttons**:
  - Clear: Deselects all files
  - Delete/Trash: Moves selected files to trash (platform-specific terminology)

## Visual Cues and Styling

The component uses color-coded backgrounds and icons to provide clear visual feedback about file status:

### File Status Indicators

#### Keep Files (Preserved)
- **Background**: Light blue (`bg-primary/5`)
- **Purpose**: Indicates files designated to be kept (the "master" copy in each duplicate group)
- **Condition**: `item.is_keep === 1`

#### Selected for Deletion
- **Background**: Light red (`bg-error/7`) with darker red hover (`hover:bg-error/10`)
- **Border/Icon**: Checkbox checked
- **Purpose**: Shows files that will be deleted when the trash action is executed
- **Condition**: File is selected for deletion via checkbox

#### Protected Files (Cannot Delete)
- **Background**: Default (no special background)
- **Icon**: Lock icon instead of checkbox
- **Purpose**: Files that cannot be selected for deletion due to protection rules
- **Conditions**:
  - Keep files (`is_keep === 1`)
  - Files outside the selected album when album-specific deletion is enabled

#### Regular Files (Selectable)
- **Background**: Default with hover effect (`hover:bg-base-200/70`)
- **Icon**: Checkbox (unchecked)
- **Purpose**: Files that can be selected for deletion
- **Condition**: Meets deletion criteria and is not protected

### Badge System

#### Keep Badge
- **Style**: Primary color badge
- **Text**: "Keep"
- **Location**: Next to filename
- **Purpose**: Clearly marks the designated keep file in each group

#### Protected Badge
- **Style**: Outline badge
- **Text**: "Protected"
- **Location**: Next to filename
- **Purpose**: Indicates files protected from deletion

### Color Scheme
- **Primary Blue**: Used for keep files and primary actions
- **Error Red**: Used for deletion candidates and destructive actions
- **Base/Content Colors**: Used for neutral states and text

This color coding provides immediate visual feedback, helping users understand which files will be preserved, which will be deleted, and which are protected from modification.

## Folder Comparison Features

In addition to library-based duplicate detection, the component supports comparing two arbitrary directories, providing a more general-purpose folder comparison tool.

### Folder Comparison Mode
- **Directory Selection**: Manual browse dialogs for selecting source and target folders
- **Comparison Scope**: Compares entire directory trees recursively, rather than being limited to a single photo library
- **Obsolete Library Mode**: When folder comparison is enabled, album-based filtering becomes obsolete as the focus shifts to direct folder-to-folder matching

### Custom Ignore Patterns
- **User-Defined Patterns**: Input field for adding wildcard patterns (e.g., *.txt, temp*, .git)
- **Pre-Loaded Defaults**: Common exclusions like *.pyc, __pycache__, .git, .svn, .DS_Store, Thumbs.db, node_modules
- **Pattern Matching**: Applies to filenames and directory paths, allowing entire branches to be filtered
- **Management**: Add/remove patterns via buttons, with scrollable list display

### Multiple Comparison Criteria
- **Name Comparison**: Exact filename matching (default enabled)
- **Size Comparison**: Byte-for-byte size equality (default enabled)
- **Timestamp Comparison**: Modification date matching with 1-second tolerance
- **Content Comparison**: Binary file content comparison for thorough validation
- **Flexible Configuration**: Mix and match criteria; at least one must be enabled

### Dual Tree Views
- **Side-by-Side Layout**: Hierarchical views mirroring source (left) and target (right) folder structures
- **Columns**: Name, Size, Modified (YYYY-MM-DD HH:MM:SS format), Similar (checkboxes)
- **Expansion/Collapse**: Full navigation of nested directories
- **Similarity Indicators**: Automatic checkbox selection for matching files based on criteria
- **Manual Overrides**: Users can check/uncheck for custom selection

### Target-Specific Deletion
- **Selective Removal**: Deletes only from the target folder, preserving source folder integrity
- **Confirmation Dialog**: "Are you sure you want to move checked items from the target folder to the recycle bin?"
- **Recycle Bin Usage**: Safe deletion with recovery options, not permanent erasure
- **Error Handling**: Collects and displays errors (e.g., permission issues) without halting the process
- **Post-Deletion Refresh**: Automatic UI update to reflect changes

### Integration with Existing Features
The folder comparison features complement the existing duplicate detection while retaining core visual cues:
- **Set Keep Button**: Allows designating preferred files (not in WinMerge)
- **Delete Button**: Platform-specific labeling (Trash on macOS, Delete elsewhere)
- **Color Coding**: Maintains blue for preserved/keep files, red for deletion candidates

## Data Structures

### Duplicate Group
```typescript
{
  id: number,                    // Unique group identifier
  items: DuplicateItem[],        // All files in group
  keepItem: DuplicateItem | null, // Designated keep file
  duplicateItems: DuplicateItem[], // Non-keep files
  reclaimableBytes: number,      // Space that can be freed
  file_count: number,           // Total files in group
  total_size: number,           // Total size of all files
  file_size: number            // Size of keep file
}
```

### Duplicate Item
```typescript
{
  file_id: number,              // File identifier
  is_keep: number,              // 1 if designated keep file, 0 otherwise
  file: {
    id: number,
    name: string,
    file_path: string,
    size: number,
    width?: number,
    height?: number,
    created_at: string,
    file_type: number,          // 1=image, 2=video, 3/4=RAW
    album_id: number,
    album_name: string,
    thumbnail?: string
  }
}
```

### Component State
- `selectedDupIdsByGroup`: Map of group IDs to sets of selected file IDs
- `mediaTypeFilter`: Current media type filter ('all' | 'image' | 'raw' | 'video')
- `deleteAlbumId`: Selected album for deletion operations
- `rawGroups`: Array of group data from backend
- `selectedGroupId`: Currently selected group ID
- `albumRootPaths`: Cache of album root paths

## Core Logic

### Duplicate Detection Process

1. **Scan Trigger**: Component watches `dedupScanKey` prop for changes
2. **Backend Communication**:
   - Calls `dedupStartScan()` with query parameters
   - Polls `dedupGetScanStatus()` for progress updates
   - Listens for scan progress events via `listenDedupScanProgress()`
3. **Data Fetching**:
   - `dedupListGroups()` retrieves groups with limit (200 max)
   - `hydrateAlbumRootPaths()` caches album paths
   - `hydrateGroupThumbnails()` generates/fetches thumbnails
4. **UI Updates**: Updates statistics and group list as scan progresses

### Filtering Logic

#### Media Type Filtering
```typescript
function fileMatchesMediaFilter(file) {
  const fileType = getFileType(file);
  switch (mediaTypeFilter) {
    case 'all': return true;
    case 'image': return fileType === 1;
    case 'video': return fileType === 2;
    case 'raw': return fileType === 3 || fileType === 4;
    default: return true;
  }
}

function groupMatchesMediaFilter(group) {
  return group.items.some(item => fileMatchesMediaFilter(item.file));
}
```

Groups are included if at least one file matches the filter.

### Selection and Protection Logic

#### File Protection Rules
- **Keep files** (`is_keep === 1`) are always protected
- **Album-specific protection**: When an album is selected for deletion, only files in that album can be selected
- Files outside the selected album are protected, even if they are duplicates

#### Selection Functions
- `canDeleteItem(item)`: Checks if item can be selected for deletion
- `isProtectedItem(item)`: Checks if item is protected from selection
- `toggleDupSelected(groupId, fileId)`: Toggles selection state
- `selectGroupDuplicates(groupId)`: Selects/deselects all deletable duplicates in group

#### Album-Based Selection
`selectDeleteAlbumDuplicates()`:
1. For each group with duplicates in target album:
2. If keep file is in target album, move keep marker to a protected file
3. Auto-select all deletable duplicates in target album

### Deletion Process

1. **Validation**: Ensures selected files are not protected
2. **Emit Event**: Sends `'trash-selected-duplicates'` with:
   - Group ID string
   - Array of selected file IDs
   - Total reclaimable bytes
3. **Backend Handling**: Parent component handles actual file operations

### Keep File Management

- `setKeep(groupId, fileId)`: Designates a file as the keep file
- Automatically deselects the file from deletion candidates
- Updates group data and refreshes UI

## Event Handling

### Emitted Events
- `close`: Close the duplicate pane
- `select-file`: Select a file in the main view
- `preview-file`: Open file preview
- `compare-group`: Open group comparison view
- `trash-selected-duplicates`: Request deletion of selected files

### Watched Properties
- `dedupScanKey`: Triggers duplicate scan when changed
- `filteredDuplicateGroups`: Updates selected group when list changes
- `albumOptions`: Clears album selection if invalid
- `deleteAlbumId`: Clears file selections when album changes
- `selectedGroupId`: Emits select-file for keep file

## Backend API Integration

### Key API Functions
- `dedupStartScan(params)`: Initiates duplicate scan
- `dedupGetScanStatus()`: Gets scan progress
- `dedupGetOverview()`: Gets total statistics
- `dedupListGroups(page, limit, sort, filter)`: Lists duplicate groups
- `dedupSetKeep(groupId, fileId)`: Sets keep file
- `getAlbum(albumId)`: Gets album details
- `getFileThumb(...)`: Generates file thumbnails

### Event Listening
- `listenDedupScanProgress(callback)`: Real-time scan progress updates

## Performance Optimizations

1. **Pagination**: Limits groups to 200 for UI responsiveness
2. **Lazy Loading**: Thumbnails loaded asynchronously
3. **Caching**: Album root paths cached in memory
4. **Debouncing**: Scan status polling at 1-second intervals
5. **Computed Properties**: Efficient filtering and calculations

## Error Handling

- Network errors logged to console
- Graceful degradation when thumbnails fail to load
- Automatic retry for failed operations
- State cleanup on component unmount

## Platform-Specific Behavior

- **macOS**: Delete button shows "Move to Trash"
- **Windows/Linux**: Delete button shows "Delete"
- Thumbnail generation adapts to platform-specific paths
- File operations use platform-appropriate trash/recycle bin

This component provides a comprehensive interface for duplicate file management, balancing user control with data safety through its protection mechanisms.