/**
 * XML Formatter Utility
 * 
 * Provides XML formatting functionality with proper indentation.
 * Uses browser-native DOMParser for parsing and formatting.
 */

/**
 * Format XML string with proper indentation
 * 
 * @param xmlString - Raw XML content as string
 * @param indent - Number of spaces for indentation (default: 2)
 * @returns Formatted XML string with proper indentation
 * 
 * @example
 * ```typescript
 * const raw = '<root><child>text</child></root>';
 * const formatted = formatXml(raw);
 * // Returns:
 * // <root>
 * //   <child>text</child>
 * // </root>
 * ```
 */
export function formatXml(xmlString: string, indent: number = 2): string {
  try {
    // Parse XML using browser DOMParser
    const parser = new DOMParser();
    const xmlDoc = parser.parseFromString(xmlString, 'text/xml');
    
    // Check for parse errors
    const parseError = xmlDoc.querySelector('parsererror');
    if (parseError) {
      // If parsing fails, return raw XML
      console.warn('XML parsing failed, returning raw content:', parseError.textContent);
      return xmlString;
    }
    
    // Format the XML with indentation
    return prettyPrintXml(xmlDoc, indent);
  } catch (error) {
    // On any error, return raw XML
    console.warn('XML formatting failed:', error);
    return xmlString;
  }
}

/**
 * Pretty print XML document with indentation
 * 
 * @param xmlDoc - Parsed XML document
 * @param indent - Number of spaces for indentation
 * @returns Formatted XML string
 */
function prettyPrintXml(xmlDoc: Document, indent: number): string {
  const serializer = new XMLSerializer();
  const xmlString = serializer.serializeToString(xmlDoc);
  
  // Format with indentation
  return formatXmlString(xmlString, indent);
}

/**
 * Format XML string with indentation
 * 
 * @param xml - XML string to format
 * @param indent - Number of spaces for indentation
 * @returns Formatted XML string
 */
function formatXmlString(xml: string, indent: number): string {
  const PADDING = ' '.repeat(indent);
  const reg = /(>)(<)(\/*)/g;
  
  // Add line breaks between tags
  let formatted = xml.replace(reg, '$1\n$2$3');
  
  // Add indentation
  let level = 0;
  const lines = formatted.split('\n');
  const result: string[] = [];
  
  for (const line of lines) {
    const trimmed = line.trim();
    
    // Skip empty lines
    if (trimmed.length === 0) continue;
    
    // Decrease level for closing tags (unless it's a self-closing tag or same-line closing)
    if (trimmed.startsWith('</')) {
      level--;
    }
    
    // Add indented line
    result.push(PADDING.repeat(Math.max(0, level)) + trimmed);
    
    // Increase level for opening tags (unless it's a self-closing tag or has closing tag on same line)
    if (trimmed.startsWith('<') && 
        !trimmed.startsWith('</') && 
        !trimmed.startsWith('<?') && 
        !trimmed.startsWith('<!') && 
        !trimmed.endsWith('/>') &&
        !trimmed.includes('</')) {
      level++;
    }
  }
  
  return result.join('\n');
}

/**
 * Check if string is valid XML
 * 
 * @param xmlString - String to validate
 * @returns true if valid XML, false otherwise
 */
export function isValidXml(xmlString: string): boolean {
  try {
    const parser = new DOMParser();
    const xmlDoc = parser.parseFromString(xmlString, 'text/xml');
    const parseError = xmlDoc.querySelector('parsererror');
    return !parseError;
  } catch {
    return false;
  }
}
