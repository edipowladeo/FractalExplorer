#include <math.h>
#include <SFML/Graphics.hpp>
#include <iostream>
#include <string>
#include <thread>
#include <Windows.h> //FreeConsole(0;

#include "OpenCLHost.h"

//headers opencl
#pragma comment(lib, "x64/OpenCL.lib") 
#define CL_USE_DEPRECATED_OPENCL_1_2_APIS
#ifdef __APPLE__
#include <OpenCL/opencl.h>
#else
#include <CL/cl.h>
#endif
#define MEM_SIZE (128)
#define MAX_SOURCE_SIZE (0x100000)

//fim headers opencl


#include <set>

//manter celula viva enquanto houverem tarefas pendentes
// ou apagar celula e lidar com o ponteiro!

#include "Cpaleta.h"
#include "Ccelula.h"
#include "global_headers.h"

#define INICIAL_OPENCL_ATIVO false



/*BUGS CONHECIDS
celula [i][j] deixando dangling pointer; false SEI SE O BUG EXISTE AINDAkkk!!
retorna para cas (tecla I ) só funciona quando está perto de casa
*/

#include "Cpaleta.h"
/*
PRIORIDADES
DEFINIR SHADER OUT = A*C^B
THREADS E FILA DE TAREFA
LIGAR E DESLIGAR SHADERS COM FACILIDADE
LIGAR E DESLIGAR DEBUG COM FACILIDADE
REMOVER CELULAS INUTEIS -> false FUNCIONA PERFEITAMENTE
PERMITIR RESOLUCAO < 1  (rever sistema de camadas)
FIXED POINTS
FIXED POINTS NA GPU
SISTEMA DE PRECISAO ARBITRARIA COM PONTEIRO E ALOCACAO DINAMICA
CONFIGURAÇÕES PRECISAS DO OPENCL
UTILIZAR MAIS DE UM DISPOSITIVO OPENCL
JULIA SET
MULTIPLAS JANELAS
ORBIT TRAP - VIA SHADER COM UNICA TEXTURA
OTIMIZAR TOUCHSCREEN
*/
//manter centro da tela no centro ao redimensionar
// alterar troca_coluna para receber N trocas e remover loop

//MOVER TODAS AS CHAMADAS DE celula_esta_dentro para Posiciona_celulas_tela e salvar em variaveis bool;

// mover todos os calculos de intervalos I J para Posiciona_celulas_tela e salver variaveis de intervalo

// mudar o nome de Posiciona_celulas_tela para calculaCoordenadas

// mudar comportamento do controle de quadros por segundo, e definir tempo minimo para processamento

//troca coluna precisa apagar o objeto celula ou basta remover ponteiro da matriz, deixando ele no vetor de ponteiros??

// mudar shader.setUniform("transparencia", (float)(Propriedades.DebugCelulas ? 0.4 : 0.0)); da funcao plota sprites para funcao toggle_debug;

//salvar ultimo tamanho da janela para restaurar quando sair de  fullscreen

//Alterar Delta Atual e desejado ao toggle_debug

//remover artificios dos sprites "transparentes"

//Adicionar Sprite placeholder e placeholder para centro da camada

// SMOOTH CAUSA ARTIFICIOS EM BAIXO DO SPRITE DEVIDO A PALETA DE CORES
// matriz iteracoes float apenas 24 bit de precisao
//CRIAR SISTEMA DE PIVOTAMENTO
//PROGRAMA PRINCIPAL PODE SER OBJETO UNICO OU PODERAO HAVER MAIS DE UMA INSTANCIA DE TPROGRAMA
/*
Tam_textura propriedade da janela
num linhas e colunas propriedade dinamica da camada

Trocar Ponteiro para ponteiro por ponteiro compartilhado share_ptr? talvez? para que quando a celula seja apagada em oturo lugar, o ponteiro Cel aponte para nulo automaticamente.

REMOVER TAREFA FRONT SE O SPRITE SAIR DA JANELA,
CRIAR NOVA TAREFA QUANDO ELE ENTRAR??

criar Celula-> Dados tarefa front;

mantar tarefa front pro final da fila
Janelas de alocação, desalocação de tarefas nao executadas, e desalocação de tarefas executadas

Alinhar camadas - CONFERIR RECOBRIMENTO

verificar se a celular nao foi mudada de lugar antes de colocar == visivel
revisitar funcao verifica mouse e alterar os tipos de dados

apagar_celulas e alocar não podem ter intervalos "janelas" conflitantes, para nao criar um looop,


vARIAS Cjanelas na mesma janela SFML


definir prioridades para tarefas(heuristico)

usar fmod no shader;

CONFIGURACOES DA JANELA
alterar tipo de dado do calculo a partir de X iterações.?? sera que é boa ideia e qual seria x=f(?)

posicao do sprite nao pode ser inteiro

Selecionar Precisão automatiSubente
precisao dinamica de coordenadas

zoom teclas está interrompendo pan!

multithread e devolver resultados das tarefas;
gpu em plano de fundo
GPGPU otimizacoes de kernel
opencl resolucoes intermediarias
optimizacao assembly???
mascara de refinamento convolucao;
editor de paletas
quebrar tarefa em iterações!!!
diminuir precisao da textura precisa de 8 bytes??
salvar coordenadas e do  julia set
*/

class Ccelula;
class Csubcamada;
class CtarefaFront;
class TPaleta;
class Cjanela;
class Cprograma;
class CbenchMark;

Cprograma Programa_principal;

using namespace std;

class Cretangulo
{
public:
	Cvetor2d minimo;
	Cvetor2d maximo;
};



Cretangulo Limites_janela_desejada;
int flag_bt_direito_mouse = false;
int flag_bt_esquerdo_mouse = false;
int flag_mouse_arrastou = false;
sf::Vector2i Coordenadas_inicio_arrastar;
sf::Vector2i Cursor_quadro_anterior;

double double_min = pow(2, -1074);

int flooredDivision(int dividendo, int divisor)
{
	if(dividendo < 0)
	{
		dividendo -= divisor * ((dividendo / divisor) - 1);
	}
	return dividendo % divisor;
}

void calcula_matriz_iteracoes_cpu(Ccelula* Cel, int resolucao, int imax)
{
	int x, y;//,pidex,cor;
	double x0, y0, cx, cy, delta;
	//  Ccoordenadas Calcular;
	x0 = (Cel->coordenadas_canto.x);
	y0 = (Cel->coordenadas_canto.y);
	delta = Cel->coordenadas_canto.delta;
	for (x = 0; x < largura_textura; x += resolucao)
	{
		for (y = 0; y < altura_textura; y += resolucao)
		{
			//           if (Cel->matriz_iteracoes[x][y] == 0)

			{
				cy = y0 + y * delta;
				cx = x0 + x * delta;
				// unsigned int
				//     Cel->matriz_iteracoes[x][y] = iteracoes_normalizada(cx,cy,imax);
				//FLOAT
				Cel->IT[x + y * largura_textura] = funcoes_mandelbrot::iteracoes_normalizada_float(cx, cy, imax);
			}
		}
	}
}

int calcula_matriz_iteracoes_opencl_temp(Ccelula* Cel, int resolucao, int i_max)
{
	////    int tamanho_vetor = largura_textura*altura_textura;
	////    int x,y,pidex;//,cor;
	////    float x0,y0,delta;
	////
	////    float CX[tamanho_vetor],CY[tamanho_vetor],IMAX[tamanho_vetor];//,it[tamanho_vetor];
	////    float *Vetores_entrada[3];
	////    float *Vetores_saida[1];
	////    Vetores_entrada[0] = CX;
	////    Vetores_entrada[1] = CY;
	////    Vetores_entrada[2] = IMAX;
	////    Vetores_saida[0] = Cel->IT;
	////
	////    x0 = (Cel->coordenadas_canto.x);
	////    y0 = (Cel->coordenadas_canto.y);
	////    delta = Cel->coordenadas_canto.delta;
	////    for (x=0; x<largura_textura; x++)
	////    {
	////        for (y=0; y<altura_textura; y++)
	////        {
	////            pidex = x+y*largura_textura;
	////            CX[pidex] = x0 + x*delta;
	////            CY[pidex] = y0 + y*delta;
	////            IMAX[pidex] = i_max;
	////        }
	////    }
	//    Benchmark1.inicio(1);
	//    Programa_principal.HostMAIN->Executar_kernel(&Vetores_entrada[0],&Vetores_saida[0]);
		 //  Benchmark1.fim(1);

	return true;
}

void lista_tarefas(Cjanela* Janela) //relatorio de texto com as Subcamadas
{
	//    int contador = 1;
	//    CtarefaFront * Tarefa_atual = Janela->Primeira_tarefa;
	//    printf("\n\nLISTA DE TAREFAS:");
	//    while (Tarefa_atual != nullptr)
	//    {
	//        printf("\nTarefa endereco %x, proxima %x, i: %d j: %d, resolucao %d",Tarefa_atual,Tarefa_atual->Proxima,Tarefa_atual->i,Tarefa_atual->j, Tarefa_atual->resolucao);
	//        printf("\nSubcamada %x, Celula %", Tarefa_atual->Sub, Tarefa_atual->Cel);
	//        cout << ", Coordenadas canto X" << *(Tarefa_atual ->Cel)->coordenadas_canto.X;
	//        Tarefa_atual =  Tarefa_atual->Proxima;
	//    }
	//    printf("\nULTIMA %x, PRIMEIRA %x",Janela->Ultima_tarefa,Janela->Primeira_tarefa);
}

int alocar_celula_e_preparar(Csubcamada* SubCamada, int i, int j, CcoordenadasPlano Coordenadas)
{
	Ccelula** PtrCelula = SubCamada->PtrCelulas[i][j];

	if (*PtrCelula == nullptr)
	{
		Programa_principal.CronometroAlocarCelula.inicia();
		try { *PtrCelula = new Ccelula(SubCamada, Coordenadas); }
		catch (const std::bad_alloc)
		{
			cout << "\nBAD ALLOC CELULA\n";
			getchar();
		}
		Programa_principal.CronometroAlocarCelula.paraeVolta();

		Programa_principal.CronometroPrepararCelula.inicia();
		if (SubCamada == nullptr) { cout << "NULL CAMADA alocar_celula_e_prepara "; }
		CTarefaAlocarCelula Tarefa(SubCamada, i, j, Coordenadas, &Programa_principal);
		Tarefa.executar();
		Programa_principal.CronometroPrepararCelula.paraeVolta();
	}
	if (((*PtrCelula)->TarefaFrontDaCelula == nullptr) && (SubCamada->JanelaMae->celula_esta_dentro(*PtrCelula, SubCamada->JanelaMae->Propriedades.JanelaDesenho))) {
		try {
			(*PtrCelula)->TarefaFrontDaCelula = new CtarefaFront(SubCamada, i, j, 1, &Programa_principal);
			// cout << endl << (*PtrCelula)->TarefaFrontDaCelula << " Criada";
		}
		catch (const std::bad_alloc) {
			cout << "\nBAD ALLOC TAREFA\n";
			getchar();
		}
		//    cout << endl << "TEORICAMENTE CRIOU TAREFA FRONT " << SubCamada->JanelaMae->NumeroDeTarefasFront();
	}
	return true;
}

int alocar_celula(Csubcamada* SubCamada, int i, int j, CcoordenadasPlano Coordenadas)
{
	if (*SubCamada->PtrCelulas[i][j] == nullptr) //MUITO PROVAVELMENTE A CELULA JÁ ESTA ALOCADA
	{
		Programa_principal.CronometroAlocarCelula.inicia();
		try { *SubCamada->PtrCelulas[i][j] = new Ccelula(SubCamada, Coordenadas); }
		catch (const std::bad_alloc)
		{
			cout << "\nBAD ALLOC CELULA\n";
			getchar();
		}
		Programa_principal.CronometroAlocarCelula.paraeVolta();
		
		CTarefaAlocarCelula* TarefaPrepararCelula;
		TarefaPrepararCelula = new CTarefaAlocarCelula(SubCamada, i, j, Coordenadas, &Programa_principal);
		SubCamada->TarefasAlocarCelulaPresentes.push_back(TarefaPrepararCelula);

		// mover push back para dentro do construtor????
		//   new CTarefaAlocarCelula(Sub,i,j,Coordenadas,&Programa_principal);


		// TarefaPrepararCelula->executar();
		// delete TarefaPrepararCelula;
	}
	return true;
}

void lista_Subcamadas(Cjanela* Janela) //relatorio de texto com as Subcamadas
{
	
}

void lista_celulas(Cjanela* Janela) //relatorio de texto com as Subcamadas
{

}

int AlocarCelulasVisiveisTimed(Cjanela* Janela, CCronometro* Cronometro, long long int tempoLimite)//Aloca novas células
{
	for (auto Itr = Janela->Camadas.begin(); Itr != Janela->Camadas.end(); Itr++)
	{
		Csubcamada* Sub = (*Itr).second;
		
		//Janela->ssdebug << endl << "AAAimin " << Sub->IntervaloAlocar.min.x << " imax " << Sub->IntervaloAlocar.max.x;
		//Janela->ssdebug << "jmin " << Sub->IntervaloAlocar.min.y << " jmax " << Sub->IntervaloAlocar.max.y;

		for (int ij = Sub->IntervaloAlocar.min.y; ij < Sub->IntervaloAlocar.max.y; ij++)
		{
			for (int ii = Sub->IntervaloAlocar.min.x; ii < Sub->IntervaloAlocar.max.x; ii++)
			{
				if (Janela->NumeroDeTarefasFront() > MAX_QUEUE_TAREFAS_FRONT) return 0;
				int i = ii - COLUNAS_DE_SPRITES / 2;
				int j = ij - LINHAS_DE_SPRITES / 2;

				CcoordenadasPlano Coordenadas_celula;
				Coordenadas_celula.delta = Sub->Coordenadas.delta;

				Coordenadas_celula.x = Sub->Coordenadas.x + i*(largura_textura - SOBREPOSICAO)* Coordenadas_celula.delta;
				Coordenadas_celula.y = Sub->Coordenadas.y + j*(altura_textura - SOBREPOSICAO)* Coordenadas_celula.delta;

					alocar_celula_e_preparar(Sub, ii, ij, Coordenadas_celula);

				if (Cronometro->tempoagora() > tempoLimite) {
					//  cout << "timeuout";
					return 0;
				}
			}
		}
	}
}

// veririfcar como fica
Ccelula** ptr_Celula_transposta(Csubcamada* Sub, int transpor, int i, int j)
{
	//DEPRECATED
	if (transpor == true) return Sub->PtrCelulas[j][i];
	else return Sub->PtrCelulas[i][j];
}

void troca_coluna_3(Cjanela *Janela, Csubcamada *Sub,int trocas_i,int trocas_j)
{
	int i, j, celula_desalocar;

		trocas_i *= -1;
		int i_min = + trocas_i;
		int i_max = COLUNAS_DE_SPRITES + trocas_i - 1;

		trocas_j *= -1;
		int j_min = +trocas_j;
		int j_max = LINHAS_DE_SPRITES + trocas_j - 1;


		Sub->offset_i += trocas_i;
		Sub->Coordenadas.x += trocas_i * Sub->Coordenadas.delta * (largura_textura - SOBREPOSICAO);
		
		Sub->offset_j += trocas_j;
		Sub->Coordenadas.y += trocas_j * Sub->Coordenadas.delta * (altura_textura - SOBREPOSICAO);

		for(int i = 0; i < COLUNAS_DE_SPRITES; i++)
		{
			for(int j = 0; j < LINHAS_DE_SPRITES; j++)
			{
				if((i<i_min)||(i>i_max)||(j<j_min)||(j>j_max))
				{
					Ccelula **PtrCelula = Sub->PtrCelulas[i][j];
					Ccelula *Celula = *PtrCelula;
					if(Celula != nullptr)
					{
						Celula->marcadaParaDestruicao = true;
					}
					*(PtrCelula) = nullptr;
				}
			}
		}

	for(i = 0; i < COLUNAS_DE_SPRITES; i++) //redistribui ponteiros
	{
		for(j = 0; j < LINHAS_DE_SPRITES; j++)
		{
			Sub->PtrCelulas[i][j] = &Sub->Celulas[flooredDivision(i + Sub->offset_i, COLUNAS_DE_SPRITES)][flooredDivision(j + Sub->offset_j, LINHAS_DE_SPRITES)];
		}
	}
}

int recalcula_borda(Csubcamada* Sub)
{
	Cjanela* Janela = Sub->JanelaMae;

	CCoordenadas2DTela CoordSubcamada = Janela->CoordenadasPlanoAtualParaTela(Sub->Coordenadas);
	CCoordenadas2DTela Coord_tela;
	
	float delta = Sub->Coordenadas.delta / Janela->DeltaDesejado;
	//CCoordenadas2DTela CoordJanela = Janela->CoordenadasPlanoAtualParaTela(Janela->pivoAtual);
	Coord_tela.x = ((CoordSubcamada.x - Janela->Propriedades.DimensoesJanela.x / 2) / delta / largura_textura);
	Coord_tela.y = ((CoordSubcamada.y - Janela->Propriedades.DimensoesJanela.y / 2) / delta / altura_textura);
	
	int trocas_i = trunc((CoordSubcamada.x - Janela->Propriedades.DimensoesJanela.x / 2) / delta / largura_textura);
	int trocas_j = trunc((CoordSubcamada.y - Janela->Propriedades.DimensoesJanela.y / 2) / delta / altura_textura);

	//Sub->JanelaMae->ssdebug << endl << " X " << Coord_tela.x << " Y " << Coord_tela.y;
	Sub->JanelaMae->DebugObjects.ssdebug << endl << "TROCAS X " << trocas_i << " Y " << trocas_j;
	
	troca_coluna_3(Janela, Sub,trocas_i,trocas_j);
	return 0;
	
}

void apagar_todas_tarefas(Cjanela* Janela)
{

	/*
	CtarefaFront *Proxima_tarefa;
	while (Janela->Primeira_tarefa != nullptr)
	{
		 Proxima_tarefa = Janela->Primeira_tarefa->Proxima;
		 delete Janela->Primeira_tarefa;
		 Janela->Primeira_tarefa = Proxima_tarefa;
	}*/
}

int verifica_teclas(Cjanela* Janela, set<retornoGUI>& Retorno)
{
	//RETORNA true CASO HAJA ALTERAÇÃO NA CÂMERA
	sf::Event event;
	while (Janela->JanelaSFML->pollEvent(event))
	{
		if (event.type == sf::Event::Closed) Janela->JanelaSFML->close();
		if (event.type == sf::Event::Resized)
		{
			sf::Vector2u Tamanho;
			Tamanho = Janela->JanelaSFML->getSize();
			Janela->Propriedades.DimensoesJanela.x = Tamanho.x;
			Janela->Propriedades.DimensoesJanela.y = Tamanho.y;
			cout << "\n REDIMENSIONOU";
		}
		/*           if (event.type== sf::Event::Resized){
				// Janela->altura = Janela->JanelaSFML->getSize();
				 cout << "\n REDIMENSIONOU";
			}*/
		if (event.type == sf::Event::KeyPressed)
		{
			if (event.key.code == sf::Keyboard::Right)
			{
				Janela->pivoDesejado.x += Janela->DeltaDesejado * Janela->Propriedades.TaxaPan;
				Retorno.insert(alterouPosicao);
				return true;
			}
			if (event.key.code == sf::Keyboard::Left)
			{
				Janela->pivoDesejado.x -= Janela->DeltaDesejado * Janela->Propriedades.TaxaPan;
				Retorno.insert(alterouPosicao);
				return true;

			}
			if (event.key.code == sf::Keyboard::Down)
			{
				Janela->pivoDesejado.y += Janela->DeltaDesejado * Janela->Propriedades.TaxaPan;
				Retorno.insert(alterouPosicao);
				return true;
			}
			if (event.key.code == sf::Keyboard::Up)
			{
				Janela->pivoDesejado.y -= Janela->DeltaDesejado * Janela->Propriedades.TaxaPan;
				Retorno.insert(alterouPosicao);
				return true;
			}
			if (event.key.code == sf::Keyboard::Z)
			{
				Janela->DeltaDesejado *= Janela->Propriedades.TaxaZoom;
				Janela->alterarPivoTela(Janela->Propriedades.DimensoesJanela.x / 2, Janela->Propriedades.DimensoesJanela.y / 2);
				Retorno.insert(alterouZoom);
				return true;
			}
			if (event.key.code == sf::Keyboard::A)
			{
				Janela->DeltaDesejado /= Janela->Propriedades.TaxaZoom;
				Janela->alterarPivoTela(Janela->Propriedades.DimensoesJanela.x / 2, Janela->Propriedades.DimensoesJanela.y / 2);
				Retorno.insert(alterouZoom);
				return true;
			}


			if(event.key.code == sf::Keyboard::Y)
			{
				Janela->Propriedades.DebugCelulas = !Janela->Propriedades.DebugCelulas;
			}			
			if(event.key.code == sf::Keyboard::T)
			{
				Janela->Propriedades.DebugCamadas = !Janela->Propriedades.DebugCamadas;
				//	troca_coluna_3(Janela, (*--Janela->Camadas.end()).second, -1, 0);
			}			
			if(event.key.code == sf::Keyboard::R)
			{
				Janela->Propriedades.UsarShader = !Janela->Propriedades.UsarShader;
				//	troca_coluna_3(Janela, (*--Janela->Camadas.end()).second, -1, 0);
			}
			if(event.key.code == sf::Keyboard::U)
			{
				Janela->Propriedades.telaCheia = !Janela->Propriedades.telaCheia;
				
				
				Janela->JanelaSFML->close();
				Janela->JanelaSFML->create(sf::VideoMode(Janela->Propriedades.DimensoesJanela.x, Janela->Propriedades.DimensoesJanela.y, 32), "MandelCL", Janela->Propriedades.telaCheia ? sf::Style::Fullscreen : sf::Style::Default);
				//				Janela->Propriedades.DebugCamadas = !Janela->Propriedades.DebugCamadas;
				//	troca_coluna_3(Janela, (*--Janela->Camadas.end()).second, -1, 0);
			}
			if(event.key.code == sf::Keyboard::I)
			{
				Janela->DeltaDesejado = Janela->Propriedades.PosicaoInicial.delta;
				Janela->pivoDesejado = Janela->Propriedades.PosicaoInicial;
			//	Janela->pivoTela = Janela->Propriedades.DimensoesJanela * 0.5;
				Janela ->pivoTela.x = Janela->Propriedades.DimensoesJanela.x / 2;
				Janela ->pivoTela.y = Janela->Propriedades.DimensoesJanela.y / 2;

			}

			




			if (event.key.code == sf::Keyboard::G)
			{
				Janela->Paleta.paleta_atual = (Janela->Paleta.paleta_atual + 1) % NUM_PALETAS;
				Janela->Paleta.precalcula_paleta();
				Janela->atualiza_texturas();
				return true;

			}
			if (event.key.code == sf::Keyboard::B)
			{
				Janela->Paleta.paleta_atual = (Janela->Paleta.paleta_atual - 1 + NUM_PALETAS) % NUM_PALETAS;
				Janela->Paleta.precalcula_paleta();
				Janela->atualiza_texturas();
				return true;
			}
			if (event.key.code == sf::Keyboard::H)
			{
				Janela->Paleta.circular_cores = !Janela->Paleta.circular_cores;
				return true;
			}
			if (event.key.code == sf::Keyboard::S) // so funcioan quando cria nova Subcamada
			{
				Janela->imax *= 2;
				return true;
			}

			if (event.key.code == sf::Keyboard::X) // so funcioan quando cria nova Subcamada
			{
				if (Janela->imax > 16)  Janela->imax /= 2;
				return true;
			}

			if (event.key.code == sf::Keyboard::N)
			{
				Janela->smooth = !Janela->smooth;
				for (int i = 0; i < COLUNAS_DE_SPRITES; i++)
				{
					for (int j = 0; j < LINHAS_DE_SPRITES; j++)
					{

						// Ccelula * Cel = Janela->Ultima_Subcamada->Celulas[i][j];
						// if (Cel!=nullptr) Cel->textura.setSmooth(Janela->smooth);
					}
				}


				//   cria_nova_Subcamada(Janela);
				//   AlocarCelulasVisiveis(Janela->Ultima_Subcamada);
				return true;
			}
			if (event.key.code == sf::Keyboard::D)
			{
				//lista_Subcamadas(Janela);
//                Programa_principal.atualiza_lista_tarefas();

			}
			if (event.key.code == sf::Keyboard::V) // so funcioan quando cria nova Subcamada
			{
				//                Benchmark1.relatorio(2,true);
			}
			/*  if (event.key.code == sf::Keyboard::D) // so funcioan quando cria nova Subcamada
			  {
					opencl_ativo = !opencl_ativo;
					cout << "\nOPENCL ";
					if (!opencl_ativo) cout<< "false ";
					cout << "ESTA ATIVO!!!";

			  }*/
		}
		if (event.type == sf::Event::MouseWheelMoved)
		{
			sf::Vector2i Cursor_tela = sf::Mouse::getPosition(*Janela->JanelaSFML);
			Janela->alterarPivoTela(Cursor_tela.x, Cursor_tela.y);

			if (event.mouseWheel.delta > 0)Janela->DeltaDesejado /= Janela->Propriedades.TaxaZoom / 2;
			else Janela->DeltaDesejado *= Janela->Propriedades.TaxaZoom / 2;
			Retorno.insert(alterouZoom);
			// cria_nova_Subcamada(Janela,gui);
			// AlocarCelulasVisiveis(Janela->Ultima_Subcamada);

		}
		if (event.type == sf::Event::MouseButtonPressed)
		{
			//  printf("\n\n\nApertouuu!!!!;;;;;;;;;;;;;");
		}
		//        gui->handleEvent(event);
	}
	return false;
}

void verifica_mouse(Cjanela* Janela)
{
	/*   sf::Vector2i Posicao_touch= sf::Touch::getPosition(0,*Janela->JanelaSFML);
		printf("\nTOUCH X : %d Y: %d",Posicao_touch.x,Posicao_touch.y) ;*/

	sf::Vector2i Cursor_tela = sf::Mouse::getPosition(*Janela->JanelaSFML);
	CCoordenadas2DTela cursorTela(sf::Mouse::getPosition(*Janela->JanelaSFML));
	CCoordenadas2DPlano Cursor_plano = Janela->CoordenadasTelaParaPlanoAtual(cursorTela);

	if (!Janela->Camadas.empty())
	{
		Cvetor2d indice;
		indice = (*--Janela->Camadas.end()).second->coordenadas_plano_para_indice(Cursor_plano);
		//  printf("\n I Ultima X %f, Indice J %f",indice.x,indice.y);
		indice = (*Janela->Camadas.begin()).second->coordenadas_plano_para_indice(Cursor_plano);
		//  printf(" I Primeira X %f, Indice J %f",indice.x,indice.y);
	}
	//SE MOUSE ESTIVER DENTRO DA JANELA
	if ((cursorTela.x >= 0) && (cursorTela.y >= 0) && (Cursor_tela.x <= Janela->Propriedades.DimensoesJanela.x) && (cursorTela.y <= Janela->Propriedades.DimensoesJanela.y))
	{
		if (sf::Mouse::isButtonPressed(sf::Mouse::Left))
		{
			// cout << "\nPRESSIONADO";
			//VERIFICAR SE O CURSOR ESTA PRESSIONADO PARA INICIAR PAN OU ZOOM;
			if (flag_bt_esquerdo_mouse == false)
			{
				flag_bt_esquerdo_mouse = true;
				//     cout << "\nAPERTOU ESQ";
				Coordenadas_inicio_arrastar = Cursor_tela;
				Programa_principal.relogio_clique.restart();
				Cursor_quadro_anterior = Cursor_tela;
				Janela->pivoTela = cursorTela;
				Janela->pivoAtual = Cursor_plano;
				Janela->pivoDesejado = Cursor_plano;
				float iteracoes = funcoes_mandelbrot::iteracoes_normalizada_float(Cursor_plano.x, Cursor_plano.y, Janela->imax);
				printf("\nCoordenadas do ponto: X: %10f Y:%10f Iteracoes:%10f", Cursor_plano.x, Cursor_plano.y, iteracoes / SAMPLING_ITERACOES);
			}

			//            sf::Vector2i delta_cursor = -Cursor_tela+Cursor_quadro_anterior;
			sf::Vector2i delta_cursor_total = Coordenadas_inicio_arrastar - Cursor_tela;
			if ((delta_cursor_total.x * delta_cursor_total.x + delta_cursor_total.y * delta_cursor_total.y) > LIMIAR_ARRASTAR_PIXELS)
			{
				flag_mouse_arrastou = true; //nao havera zoom;
			}
			//PANNNN
			Janela->pivoTela = cursorTela;
			// Janela->Camera_desejada = Janela->Camera_atual;
			Cursor_quadro_anterior = Cursor_tela;
		}
		else
		{
			if (flag_bt_esquerdo_mouse == true)
			{
				//   cout << "\nSOLTOU ESQ" ;
				Programa_principal.Tempo_clique = Programa_principal.relogio_clique.getElapsedTime();
				//   cout << " depois de " << Tempo_clique.asMilliseconds();
				if (flag_mouse_arrastou == false)
				{
					if ((Cursor_tela.x >= 0) && (Cursor_tela.y >= 0) && (Cursor_tela.x <= Janela->Propriedades.DimensoesJanela.x) && (Cursor_tela.y <= Janela->Propriedades.DimensoesJanela.y))
					{

						Janela->DeltaDesejado /= Janela->Propriedades.TaxaZoom;
						//                        cria_nova_Subcamada(Janela,gui);
						//                        AlocarCelulasVisiveis(Janela->Ultima_Subcamada);
					}
				}
				flag_mouse_arrastou = false;
			}
			flag_bt_esquerdo_mouse = false;
		}

		if (sf::Mouse::isButtonPressed(sf::Mouse::Right))
		{
			if (flag_bt_direito_mouse == false)
			{
				flag_bt_direito_mouse = true;
				Janela->pivoTela = cursorTela;
				Janela->pivoAtual = Cursor_plano;
				Janela->pivoDesejado = Cursor_plano;
			}
		}
		else
		{
			if (flag_bt_direito_mouse == true)
			{
				Janela->DeltaDesejado *= Janela->Propriedades.TaxaZoom;
				//   cria_nova_Subcamada(Janela);
				//  AlocarCelulasVisiveis(Janela->Ultima_Subcamada);
				//    Retorno.insert(alterouZoom);
			}
			flag_bt_direito_mouse = false;
		}
	}
}

void transfere_tarefas_PrepararCelula(Cjanela* Janela)
{
	for (auto ItrCamada = Janela->Camadas.begin(); ItrCamada != Janela->Camadas.end(); ItrCamada++)
	{
		Csubcamada* Camada = (*ItrCamada).second;
		for (auto ItrTarefa = Camada->TarefasAlocarCelulaPresentes.begin(); ItrTarefa != Camada->TarefasAlocarCelulaPresentes.end(); ItrTarefa++)
		{
			if (Camada->TarefasFrontPresentes.size() < 5)
			{
				CTarefaAlocarCelula* Tarefa = *ItrTarefa;
				Ccelula* Celula = *Tarefa->PtrCelula;
				if (Celula == nullptr) { Tarefa->TarefaFoiExecutada = true; }
				else
				{
					if (Celula->EstaDentroDaJanelaVisivel)
					{
						Tarefa->executar();
					}
					//                    else {
					//                            missedattempts ++;
					//                    Camada->TarefasFrontPresentes.push_back(Tarefa);// Coloca Tarefa no Final da Fila
					//                    Camada->TarefasFrontPresentes.erase(ItrTarefa++);// Remove da posical atual
					//                    }
				}
				if (Tarefa->TarefaFoiExecutada)
				{
					ItrTarefa = Camada->TarefasAlocarCelulaPresentes.erase(ItrTarefa);
					if (ItrTarefa == Camada->TarefasAlocarCelulaPresentes.end()) break;
				}
			}
		}
	}
	//  cout <<endl<<"\t\t\t\t\tMISSED ATTEMPTS " << missedattempts << endl;
}


void transfere_tarefas_por_camadas(Cjanela* Janela)
{
	//    int missedattempts = 0;
	for (auto ItrCamada = Janela->Camadas.begin(); ItrCamada != Janela->Camadas.end(); ItrCamada++)
	{
		//  if (Programa_principal.TarefasBackPresentes.size()>MAX_TAREFAS_ALOCAR) return 0;
		//transfere TODAS de uma vez , muito lento
		Csubcamada* Camada = (*ItrCamada).second;
		for (auto ItrTarefa = Camada->TarefasFrontPresentes.begin(); ItrTarefa != Camada->TarefasFrontPresentes.end(); ItrTarefa++)
		{
			CtarefaFront* Tarefa = *ItrTarefa;
			Ccelula* Celula = *Tarefa->Cel;
			if (!(Programa_principal.TarefasBackPresentes.size() > MAX_TAREFAS_ALOCAR)) //return 0;
			{
				if (Tarefa->status == 0)
				{
					if (Celula == nullptr) {
						Tarefa->status = destruir;
						//  cout << endl << Tarefa << " Destruida Celula = nullptr";
					}
					else
					{
				//		if (Celula->EstaDentroDaJanelaVisivel)
						if(Janela->celula_esta_dentro(Celula, Janela->Propriedades.JanelaDesenho))
						{
							//   cout << endl << Tarefa << " Transferida";
							Tarefa->transfere();
						}
						else {
							Tarefa->status = destruir;

							//  cout << endl << Tarefa << " Destruida fora DaJanelaVisivel)";
//                            missedattempts ++;
//                    Camada->TarefasFrontPresentes.push_back(Tarefa);// Coloca Tarefa no Final da Fila
//                    Camada->TarefasFrontPresentes.erase(ItrTarefa++);// Remove da posical atual
						}
					}
				}
				if (Tarefa->status == destruir)
				{
					ItrTarefa = Camada->TarefasFrontPresentes.erase(ItrTarefa);
					if (ItrTarefa == Camada->TarefasFrontPresentes.end()) break;
				}
			}
		}
	}
	//  cout <<endl<<"\t\t\t\t\tMISSED ATTEMPTS " << missedattempts << endl;
}


void pega_tarefa_resolucao(Cjanela* Janela)
{
	//EXECUTA
	//  if  (Programa_principal.TarefasBackPresentes.front()->tarefa_fechada)
	if (!Programa_principal.TarefasBackPresentes.empty())
	{
		CtarefaBack* PrtTarefaExecutar = Programa_principal.TarefasBackPresentes.front();
		//executa se a tarefa esta fechada ou ela é a ultima e nao tem nenhuma tarea front para processar.
		if ((Programa_principal.TarefasBackPresentes.front()->tarefa_fechada) || (Programa_principal.TarefasBackPresentes.size() == 1))//&&(Programa_principal.TarefasFrontPresentes.size()==0)))
		{
			Programa_principal.TarefasBackPresentes.remove(PrtTarefaExecutar);
			//É PRA ENVIAR ASSINCRONO
			PrtTarefaExecutar->executa();
			//          cout <<"-";
			// false ESQUECER DE DELETAR
			delete  PrtTarefaExecutar;
		}
	}
}

/*
int preparar_opencl()
{
	 TListaPlataformas *ListaP = listarPlataformas();
	 cl_device_id    Dispositivo1;
	 Dispositivo1 = ListaP->Plataformas[2]->devices[0];
	 Programa_principal.HostMAIN = new THost(&ListaP->Plataformas[0]->devices[0],3,1,"itnorm1",altura_textura*largura_textura);
	 // teste();
}*/


int TarefaDesenho();

void OpenCLHelloWorld()
{
	cl_device_id device_id = NULL;
	cl_context context = NULL;
	cl_command_queue command_queue = NULL;
	cl_mem memobj = NULL;
	cl_program program = NULL;
	cl_kernel kernel = NULL;
	cl_platform_id platform_id = NULL;
	cl_uint ret_num_devices;
	cl_uint ret_num_platforms;
	cl_int ret;

	char string[MEM_SIZE];

	FILE *fp;
	char fileName[] = "./hello.c";
	char *source_str;
	size_t source_size;

	/* Load the source code containing the kernel*/
//	fp = fopen(fileName, "r");
	fp = nullptr;
	if(!fp)
	{
		fprintf(stderr, "Failed to load kernel.\n");
		exit(1);
	}
	source_str = (char *)malloc(MAX_SOURCE_SIZE);
	source_size = fread(source_str, 1, MAX_SOURCE_SIZE, fp);
	fclose(fp);

	/* Get Platform and Device Info */
	ret = clGetPlatformIDs(1, &platform_id, &ret_num_platforms);
	ret = clGetDeviceIDs(platform_id, CL_DEVICE_TYPE_DEFAULT, 1, &device_id, &ret_num_devices);

	/* Create OpenCL context */
	context = clCreateContext(NULL, 1, &device_id, NULL, NULL, &ret);

	/* Create Command Queue */
	command_queue = clCreateCommandQueue(context, device_id, 0, &ret);

	/* Create Memory Buffer */
	memobj = clCreateBuffer(context, CL_MEM_READ_WRITE, MEM_SIZE * sizeof(char), NULL, &ret);

	/* Create Kernel Program from the source */
	program = clCreateProgramWithSource(context, 1, (const char **)&source_str,
		(const size_t *)&source_size, &ret);

	/* Build Kernel Program */
	ret = clBuildProgram(program, 1, &device_id, NULL, NULL, NULL);

	/* Create OpenCL Kernel */
	kernel = clCreateKernel(program, "hello", &ret);

	/* Set OpenCL Kernel Parameters */
	ret = clSetKernelArg(kernel, 0, sizeof(cl_mem), (void *)&memobj);

	/* Execute OpenCL Kernel */
	ret = clEnqueueTask(command_queue, kernel, 0, NULL, NULL);

	/* Copy results from the memory buffer */
	ret = clEnqueueReadBuffer(command_queue, memobj, CL_TRUE, 0,
		MEM_SIZE * sizeof(char), string, 0, NULL, NULL);

	/* Display Result */
	puts(string);

	/* Finalization */
	ret = clFlush(command_queue);
	ret = clFinish(command_queue);
	ret = clReleaseKernel(kernel);
	ret = clReleaseProgram(program);
	ret = clReleaseMemObject(memobj);
	ret = clReleaseCommandQueue(command_queue);
	ret = clReleaseContext(context);

	free(source_str);

}


#include <stdio.h>
#include <stdlib.h>

#ifdef __APPLE__
#include <OpenCL/opencl.h>
#else
#include <CL/cl.h>
#endif

#define MAX_SOURCE_SIZE (0x100000)

int TesteOpencl2()
{
	printf("started running\n");

	// Create the two input vectors
	int i;
	const int LIST_SIZE = 1024;
	int *A = (int *)malloc(sizeof(int) * LIST_SIZE);
	int *B = (int *)malloc(sizeof(int) * LIST_SIZE);
	for(i = 0; i < LIST_SIZE; i++)
	{
		A[i] = i;
		B[i] = i;
	}

	// Load the kernel source code into the array source_str
	FILE *fp;
	char *source_str;
	size_t source_size;

	fopen_s(&fp, "./KernelsOpenCL/opencl_kernels.cl", "r");
	if(!fp)
	{
		fprintf(stderr, "Failed to load kernel.\n");
		getchar();
	}

	source_str = (char *)malloc(MAX_SOURCE_SIZE);
	source_size = fread(source_str, 1, MAX_SOURCE_SIZE, fp);
	fclose(fp);
	printf("kernel loading done\n");

	// Get platform and device information
	cl_device_id device_id = NULL;
	cl_uint ret_num_devices;
	cl_uint ret_num_platforms;


	cl_int ret = clGetPlatformIDs(0, NULL, &ret_num_platforms);
	cl_platform_id *platforms = NULL;
	platforms = (cl_platform_id *)malloc(ret_num_platforms * sizeof(cl_platform_id));

	ret = clGetPlatformIDs(ret_num_platforms, platforms, NULL);
	printf("ret at %d is %d\n", __LINE__, ret);

	ret = clGetDeviceIDs(platforms[1], CL_DEVICE_TYPE_ALL, 1,&device_id, &ret_num_devices);
	printf("ret at %d is %d\n", __LINE__, ret);

	// Create an OpenCL context
	cl_context context = clCreateContext(NULL, 1, &device_id, NULL, NULL, &ret);
	printf("ret at %d is %d\n", __LINE__, ret);

	// Create a command queue
	cl_command_queue command_queue = clCreateCommandQueue(context, device_id, 0, &ret);
	printf("ret at %d is %d\n", __LINE__, ret);

	// Create memory buffers on the device for each vector 

	cl_mem a_mem_obj = clCreateBuffer(context, CL_MEM_READ_ONLY,
		LIST_SIZE * sizeof(int), NULL, &ret);
	cl_mem b_mem_obj = clCreateBuffer(context, CL_MEM_READ_ONLY,
		LIST_SIZE * sizeof(int), NULL, &ret);
	cl_mem c_mem_obj = clCreateBuffer(context, CL_MEM_WRITE_ONLY,
		LIST_SIZE * sizeof(int), NULL, &ret);

	// Copy the lists A and B to their respective memory buffers
	ret = clEnqueueWriteBuffer(command_queue, a_mem_obj, CL_TRUE, 0,LIST_SIZE * sizeof(int), A, 0, NULL, NULL);
	printf("ret at %d is %d\n", __LINE__, ret);

	ret = clEnqueueWriteBuffer(command_queue, b_mem_obj, CL_TRUE, 0,LIST_SIZE * sizeof(int), B, 0, NULL, NULL);
	printf("ret at %d is %d\n", __LINE__, ret);

	printf("before building\n");
	// Create a program from the kernel source
	cl_program program = clCreateProgramWithSource(context, 1,
		(const char **)&source_str, (const size_t *)&source_size, &ret);
	printf("ret at %d is %d\n", __LINE__, ret);

	// Build the program
	ret = clBuildProgram(program, 1, &device_id, NULL, NULL, NULL);
	printf("ret at %d is %d\n", __LINE__, ret);

	printf("after building\n");
	// Create the OpenCL kernel
	cl_kernel kernel = clCreateKernel(program, "vector_add", &ret);
	printf("ret at %d is %d\n", __LINE__, ret);

	// Set the arguments of the kernel
	ret = clSetKernelArg(kernel, 0, sizeof(cl_mem), (void *)&a_mem_obj);
	printf("ret at %d is %d\n", __LINE__, ret);

	ret = clSetKernelArg(kernel, 1, sizeof(cl_mem), (void *)&b_mem_obj);
	printf("ret at %d is %d\n", __LINE__, ret);

	ret = clSetKernelArg(kernel, 2, sizeof(cl_mem), (void *)&c_mem_obj);
	printf("ret at %d is %d\n", __LINE__, ret);

	//added this to fix garbage output problem
	//ret = clSetKernelArg(kernel, 3, sizeof(int), &LIST_SIZE);

	printf("before execution\n");
	// Execute the OpenCL kernel on the list
	size_t global_item_size = LIST_SIZE; // Process the entire lists
	size_t local_item_size = 64; // Divide work items into groups of 64
	ret = clEnqueueNDRangeKernel(command_queue, kernel, 1, NULL,
		&global_item_size, &local_item_size, 0, NULL, NULL);
	printf("after execution\n");
	// Read the memory buffer C on the device to the local variable C
	int *C = (int *)malloc(sizeof(int) * LIST_SIZE);
	ret = clEnqueueReadBuffer(command_queue, c_mem_obj, CL_TRUE, 0,
		LIST_SIZE * sizeof(int), C, 0, NULL, NULL);
	printf("after copying\n");
	// Display the result to the screen
	for(i = 0; i < LIST_SIZE; i++)
		printf("%d + %d = %d\n", A[i], B[i], C[i]);

	// Clean up
	ret = clFlush(command_queue);
	ret = clFinish(command_queue);
	ret = clReleaseKernel(kernel);
	ret = clReleaseProgram(program);
	ret = clReleaseMemObject(a_mem_obj);
	ret = clReleaseMemObject(b_mem_obj);
	ret = clReleaseMemObject(c_mem_obj);
	ret = clReleaseCommandQueue(command_queue);
	ret = clReleaseContext(context);
	free(A);
	free(B);
	free(C);
	return 0;
}

int main()
{
	//OpenCLHelloWorld();
TListaPlataformas lista1 = listarPlataformas();
exibirDetalhesPlataformas(lista1);
	//getchar();
	testeOpenCL();
//TesteOpencl2();
	getchar();
//thread ThreadPrincipal(TarefaDesenho);
//ThreadPrincipal.join();
	//CCoordenadas2DI DimensoesJanela(1000, 700);

TarefaDesenho();
}
//#include "intros_ptree.hpp"


int TarefaDesenho()
{
	Cjanela* Janela = new Cjanela(&Programa_principal);

	/*
	BEGIN_INTROS_TYPE(Cjanela)
		ADD_INTROS_ITEM()
		ADD_INTROS_ITEM()
		ADD_INTROS_ITEM()
		ADD_INTROS_ITEM()
		ADD_INTROS_ITEM()
		ADD_INTROS_ITEM()
		END_INTROS_TYPE(Cjanela)



		ptree arvore;

		auto make_intros_object<Cjanela>(OIJanela);
		OIJanela.name = "novo nome";
		*/

	//  preparar_opencl();
//teste();
	sf::RenderWindow* JanelaSFML = Janela->JanelaSFML;

	// Janela->verificarSeInsereCamadas();

	while (JanelaSFML->isOpen())
	{
		Janela->relatorioCamadas();

		for(auto ItrCamada = Janela->Camadas.begin(); ItrCamada != Janela->Camadas.end(); ItrCamada++)
		{
			Csubcamada *Camada = (*ItrCamada).second;
			Camada->CalculaIntervalos(); 
			Camada->posiciona_celulas_tela();
		}

		if (Programa_principal.CronometroPrepararCelula.voltas > 0) cout << endl << "\t\t\tCEL: " << Programa_principal.CronometroAlocarCelula.voltas << " -> "
			<< Programa_principal.CronometroAlocarCelula.media() <<
			"\tTEXT: " << Programa_principal.CronometroCriarTextura.voltas <<
			" -> " << Programa_principal.CronometroCriarTextura.media() <<
			"\tPreparar" << Programa_principal.CronometroPrepararCelula.voltas <<
			" -> " << Programa_principal.CronometroPrepararCelula.media();

		Programa_principal.CronometroCriarTextura.reinicia();
		Programa_principal.CronometroAlocarCelula.reinicia();
		Programa_principal.CronometroPrepararCelula.reinicia();

		Janela->verificarSeInsereCamadas();

		set<retornoGUI> RetornosGUI;

		Programa_principal.relogio_refresh.restart(); // necessario para controlar taxa de quadros
		int        tempoantigo = 0;
		Janela->plota_sprites();
		int     tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		cout << endl << "P " << tempogora - tempoantigo << "\t";
		tempoantigo = tempogora;

		for (auto Itr = Janela->Camadas.begin(); Itr != Janela->Camadas.end(); Itr++)
		{
			int Magnificacao = (*Itr).first;
			if ((Magnificacao <= Janela->MagnificacaoMax) && (Magnificacao >= Janela->MagnificacaoMin))
			{
				(*Itr).second->MarcarCelulasInuteis();
			}
		}

		Janela->ApagarCelulasInuteis();//ESTA FUNCAO DEVE RODAR ANTES DE CRIAR NOVA CAMADA PARA evitar  calculo de células que serão apagadas
		tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		cout << "A " << tempogora - tempoantigo << "\t";
		tempoantigo = tempogora;

		verifica_teclas(Janela, RetornosGUI);
		verifica_mouse(Janela);

		tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		cout << "V " << tempogora - tempoantigo << "\t";
		tempoantigo = tempogora;

		for (auto Itr = Janela->Camadas.begin(); Itr != Janela->Camadas.end(); Itr++)
		{
			recalcula_borda((*Itr).second); //esta função faz a troca de colunas e ponteiros

		}
		tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		cout << "R " << tempogora - tempoantigo << "\t";
		tempoantigo = tempogora;

		
		CCronometro* CronometroTarefas = &Programa_principal.CronometroTarefas;
		
		Programa_principal.Cronometro1.inicia();
		CronometroTarefas->inicia();
		AlocarCelulasVisiveisTimed(Janela, CronometroTarefas, 30000);//Aloca novas células

		int tempo = Programa_principal.Cronometro1.tempoagora();
		// cout << "\n ALOCAR CELULAS LEVOU " << tempo << " (MICROSS) ";

		tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		cout << "Al " << tempogora - tempoantigo << "\t";
		tempoantigo = tempogora;

		//   Janela->verificarSeInsereCamadas();
		tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		cout << "C " << tempogora - tempoantigo << "\t";
		tempoantigo = tempogora;

		if (RetornosGUI.count(alterouZoom))
		{
			//     Janela->verificarSeInsereCamadas();
		}

		//Programa_principal.relogio_refresh.restart(); // necessario para controlar taxa de quadros
		tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
		//int tempoantigo;// = tempogora;
//        int tempoD = tempogora;
		while (CronometroTarefas->tempoagora() < 30000)
		{
			//  cout << "d " << tempoD << "\t";

			tempoantigo = tempogora;
			transfere_tarefas_PrepararCelula(Janela);
			transfere_tarefas_por_camadas(Janela);
			tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
			//    cout << "t "<< tempogora-tempoantigo << "\t";

			tempoantigo = tempogora;
			pega_tarefa_resolucao(Janela);
			tempogora = Programa_principal.relogio_refresh.getElapsedTime().asMilliseconds();
			//    cout << "E "<< tempogora-tempoantigo;
			// Programa_principal.tempo_refresh = Programa_principal.relogio_refresh.getElapsedTime();
		}
	}
	return 0;
}



