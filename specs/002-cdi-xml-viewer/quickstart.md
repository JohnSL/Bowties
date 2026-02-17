# Quickstart: CDI XML Viewer

**Feature**: 001-cdi-xml-viewer  
**For**: Developers and testers debugging LCC node configuration  
**Reading Time**: 3 minutes

---

## What is the CDI XML Viewer?

The CDI XML Viewer is a debugging tool that lets you view the raw Configuration Description Information (CDI) XML data retrieved from LCC nodes. It displays the XML in a formatted, human-readable way so you can:

- **Verify CDI retrieval** is working correctly
- **Inspect the structure** of a node's configuration capabilities
- **Debug configuration issues** by examining the raw CDI schema
- **Copy XML to external tools** for detailed analysis

> **Note**: This is a developer/debugging feature, not a primary user workflow. For normal node configuration, use the configuration editor interface.

---

## Prerequisites

Before you can view CDI XML:

1. **Connect to an LCC network** via TCP
2. **Discover nodes** on the network (node list must be populated)
3. **Retrieve configuration** for the node you want to inspect

The CDI XML viewer displays already-retrieved CDI data. It doesn't fetch CDI from nodes itself.

---

## How to View CDI XML

### Step 1: Locate the Node

Navigate to the **Nodes** page in Bowties where you can see the list of discovered LCC nodes.

### Step 2: Open Context Menu

**Right-click** on the node whose CDI you want to view. A context menu will appear with several options.

> **Keyboard alternative**: Select the node and press `Ctrl+I` (or `Cmd+I` on macOS) to open the CDI viewer directly.

### Step 3: Select "View CDI XML"

Click **"View CDI XML"** from the context menu. A modal window will open showing the formatted CDI XML.

### Step 4: Inspect the XML

The XML will be displayed with:
- ✅ **Proper indentation** showing the element hierarchy
- ✅ **Monospaced font** for easy reading
- ✅ **Scrollable content** for large documents
- ✅ **Preserved formatting** (all original content intact)

### Step 5: Copy if Needed (Optional)

Click the **"Copy"** button at the top of the modal to copy the entire XML to your clipboard. You can then paste it into:
- Text editors (VS Code, Notepad++, etc.)
- XML validators
- Configuration analysis tools
- Bug reports or support requests

### Step 6: Close the Viewer

Click **"Close"** or press **Escape** to close the modal and return to the node list.

---

## Example Workflow

```
1. Connect to LCC network (TCP: 192.168.1.100:12021)
2. Discover nodes → See "Layout Control Board #1" in list
3. Right-click on "Layout Control Board #1"
4. Select "View CDI XML"
5. Modal opens showing formatted XML:

   <?xml version="1.0"?>
   <cdi>
     <identification>
       <manufacturer>Example Corp</manufacturer>
       <model>LCB-2000</model>
       <hardwareVersion>1.0</hardwareVersion>
       <softwareVersion>2.3.1</softwareVersion>
     </identification>
     <segment space="253">
       ...
     </segment>
   </cdi>

6. Click "Copy" to copy XML
7. Paste into text editor for detailed analysis
8. Close modal
```

---

## What You'll See

### Successful View

When CDI is available, you'll see:
- **Node ID** in the modal header (e.g., "CDI XML - Node 01.02.03.04.05.06")
- **Formatted XML** with proper indentation
- **Copy** and **Close** buttons
- **Scroll bar** if content is long

### Error Messages

If CDI can't be displayed, you'll see one of these messages:

| Message | Meaning | Solution |
|---------|---------|----------|
| "CDI data has not been retrieved for this node." | Configuration hasn't been fetched yet | Retrieve node configuration first |
| "This node does not provide CDI." | Node doesn't support configuration | Normal for some node types - no action needed |
| "CDI retrieval failed: [details]" | Network or protocol error | Check connection, retry configuration retrieval |
| "XML parsing failed. Raw content shown below." | Malformed XML from node | Contact node manufacturer or submit bug report |
| "Node not found." | Node removed from list | Refresh node list |

---

## Tips & Tricks

### 🔍 **Finding Specific Elements**

Use your browser's Find feature (`Ctrl+F` or `Cmd+F`) to search within the XML display. Search for:
- Element names: `<segment>`, `<int>`, `<string>`
- Attribute names: `space=`, `size=`, `offset=`
- Configuration names

### 📊 **Large Documents**

If the CDI is very large (> 1MB), you'll see a warning:
> "Large document may impact performance"

The viewer will:
- Display the first 1000 lines
- Provide a "Load More" button to see the rest
- Always allow copying the full content via "Copy" button

### 🐛 **Debugging Configuration Issues**

When configuration isn't working as expected:
1. View the CDI XML
2. Verify the `<segment>` and memory space match what you expect
3. Check field sizes, offsets, and types
4. Compare with node documentation
5. Copy XML and share in support request if needed

### 📋 **Copying Specific Sections**

If you need just part of the XML:
1. Open the CDI viewer
2. Select the text you want (click and drag)
3. Right-click → "Copy" (or `Ctrl+C`)
4. Paste into your destination

No need to copy the entire document if you only need a snippet.

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| **Right-click on node** | Open context menu |
| `Ctrl+I` (or `Cmd+I`) | View CDI XML (when node selected) |
| `Escape` | Close CDI viewer modal |
| `Ctrl+F` (or `Cmd+F`) | Search within XML (browser feature) |
| `Ctrl+C` (or `Cmd+C`) | Copy selected text |

---

## Frequently Asked Questions

### Q: Why is "View CDI XML" grayed out?

**A**: Either:
- Configuration hasn't been retrieved for that node yet (retrieve it first)
- Node doesn't provide CDI (some node types don't support configuration)

### Q: Can I edit the CDI XML?

**A**: No, this is a read-only viewer for debugging purposes. CDI editing (if implemented) will be a separate feature with validation and safety checks.

### Q: Why does the XML look different from the node's documentation?

**A**: The CDI viewer shows the **exact XML as stored in the node**, which may:
- Have different whitespace than documentation examples
- Include optional fields not shown in simplified docs
- Use different element order (still valid XML)

If the content differs significantly, it may indicate a firmware version mismatch or node configuration issue.

### Q: Can I save the CDI XML to a file?

**A**: Currently, use the "Copy" button and paste into a text editor, then save from there. Direct file export may be added in a future update.

### Q: The XML shows strange characters or encoding issues

**A**: This usually indicates:
- Encoding mismatch (node using non-UTF-8 encoding)
- Corrupted data during retrieval
- Firmware bug in the node

Try retrieving the CDI again. If the problem persists, report it to the node manufacturer.

---

## Troubleshooting

### Problem: Modal doesn't open when I right-click

**Possible causes**:
- CDI not retrieved → Retrieve configuration first
- JavaScript error → Check browser console (F12)
- UI bug → Restart Bowties, try again

### Problem: XML is completely unreadable (no formatting)

**Possible causes**:
- Malformed XML from node → Parser can't format it
- Very old browser → Update to modern browser (shouldn't happen in Tauri)

**Workaround**: Copy the raw XML and paste into external XML formatter

### Problem: Performance is slow with large CDI

**Expected behavior**: Documents over 1MB may take a few seconds to format

**Workaround**: 
- Wait for initial formatting to complete
- Use "Copy" to get raw XML without waiting for full render
- External tools may handle very large XML better

---

## Related Features

- **Node Discovery**: Populate the node list (prerequisite for CDI viewing)
- **Configuration Retrieval**: Fetch CDI from nodes (prerequisite for CDI viewing)
- **Configuration Editor**: Edit node configuration (uses CDI schema)

---

## Feedback & Support

If you encounter issues or have suggestions for improving the CDI XML Viewer:

1. Check the [troubleshooting section](#troubleshooting) above
2. Search existing issues on GitHub
3. Submit a bug report with:
   - Node type and firmware version
   - Error message (if any)
   - Screenshot of the issue
   - Copy of the CDI XML (if relevant and not sensitive)

---

## Summary

The CDI XML Viewer is a simple but powerful debugging tool:

✅ **Right-click any node** → "View CDI XML"  
✅ **See formatted XML** with proper indentation  
✅ **Copy to clipboard** for external analysis  
✅ **Close with Escape** when done

Perfect for developers and advanced users who need to inspect node configuration schemas for debugging or integration work.

**Next Steps**: Try viewing CDI for a known node, then experiment with searching and copying specific sections!