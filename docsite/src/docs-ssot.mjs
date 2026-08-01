/** Repository-level documentation is the only authored content tree. */
export const docsSourceDirectory = "../docs";

export function docsSidebarDirectory(section) {
  return `${docsSourceDirectory}/${section}`;
}
