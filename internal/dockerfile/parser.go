package dockerfile

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// Arg represents a build argument in a Dockerfile
type Arg struct {
	Name         string
	DefaultValue string
	HasDefault   bool
}

// Dockerfile represents a parsed Dockerfile
type Dockerfile struct {
	Path string
	Args []Arg
}

// Parser parses Dockerfiles
type Parser struct{}

// NewParser creates a new Parser
func NewParser() *Parser {
	return &Parser{}
}

// Parse reads and parses a Dockerfile, extracting ARG declarations
func (p *Parser) Parse(path string) (*Dockerfile, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("failed to open Dockerfile %s: %w", path, err)
	}
	defer file.Close()

	df := &Dockerfile{
		Path: path,
		Args: []Arg{},
	}

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())

		// Skip comments and empty lines
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		// Check for ARG instruction
		if strings.HasPrefix(strings.ToUpper(line), "ARG ") {
			arg := p.parseArg(line[4:])
			df.Args = append(df.Args, arg)
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("failed to read Dockerfile %s: %w", path, err)
	}

	return df, nil
}

// parseArg parses an ARG declaration
func (p *Parser) parseArg(argStr string) Arg {
	argStr = strings.TrimSpace(argStr)

	// Check for default value (ARG name=value)
	before, after, ok := strings.Cut(argStr, "=")
	if ok {
		return Arg{
			Name:         strings.TrimSpace(before),
			DefaultValue: strings.TrimSpace(after),
			HasDefault:   true,
		}
	}

	return Arg {
		Name: argStr,
		HasDefault: false,
	}
}

// FindDockerfiles finds Dockerfiles in the current directory
func (p *Parser) FindDockerfiles() ([]string, error) {
	var dockerfiles []string

	// Check for default Dockerfile
	if _, err := os.Stat("Dockerfile"); err == nil {
		dockerfiles = append(dockerfiles, "Dockerfile")
	}

	// Check for Dockerfile.* patterns
	entries, err := os.ReadDir(".")
	if err != nil {
		return nil, fmt.Errorf("failed to read current directory: %w", err)
	}

	for _, entry := range entries {
		name := entry.Name()
		// Match Dockerfile.* pattern but not just "Dockerfile"
		if strings.HasPrefix(name, "Dockerfile.") && !entry.IsDir() {
			dockerfiles = append(dockerfiles, name)
		}
	}

	return dockerfiles, nil
}

// GetDockerfileName returns a friendly name for the Dockerfile
func GetDockerfileName(path string) string {
	if path == "Dockerfile" {
		return "default"
	}
	// Return the part after "Dockerfile."
	return strings.TrimPrefix(filepath.Base(path), "Dockerfile.")
}
