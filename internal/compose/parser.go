package compose

import (
	"fmt"
	"os"
	"sort"

	"gopkg.in/yaml.v3"
)

// ComposeFile represents a docker-compose.yaml structure
type ComposeFile struct {
	Version  string             `yaml:"version"`
	Services map[string]Service `yaml:"services"`
}

// Service represents a service in docker-compose
type Service struct {
	Image         string   `yaml:"image"`
	Build         string   `yaml:"build"`
	ContainerName string   `yaml:"container_name"`
	Ports         []string `yaml:"ports"`
	Environment   any      `yaml:"environment"`
	Volumes       []string `yaml:"volumes"`
	DependsOn     []string `yaml:"depends_on"`
}

// Parser parses docker-compose files
type Parser struct{}

// NewParser creates a new Parser
func NewParser() *Parser {
	return &Parser{}
}

// FindComposeFile finds a docker-compose file in the current directory
func (p *Parser) FindComposeFile() (string, bool) {
	candidates := []string{
		"docker-compose.yaml",
		"docker-compose.yml",
		"compose.yaml",
		"compose.yml",
	}

	for _, f := range candidates {
		if _, err := os.Stat(f); err == nil {
			return f, true
		}
	}
	return "", false
}

// Parse parses a docker-compose file and returns the services
func (p *Parser) Parse(filename string) (*ComposeFile, error) {
	data, err := os.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("failed to read compose file %s: %w", filename, err)
	}

	var compose ComposeFile
	if err := yaml.Unmarshal(data, &compose); err != nil {
		return nil, fmt.Errorf("failed to parse compose file %s: %w", filename, err)
	}

	return &compose, nil
}

// GetServiceNames returns a sorted list of service names
func (p *Parser) GetServiceNames(compose *ComposeFile) []string {
	names := make([]string, 0, len(compose.Services))
	for name := range compose.Services {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}
